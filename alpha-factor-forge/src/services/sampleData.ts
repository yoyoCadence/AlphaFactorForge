// Deterministic synthetic OHLCV, for trying the backtest path when no real
// candles are imported yet. The webview CSP blocks live exchange fetches, so a
// seeded local generator unblocks Slice 2 end to end. NOT for evaluation —
// it's a smooth-ish random walk, only good enough to exercise the pipeline.

import type { Candle as DbCandle } from '../tauri-client/commands';

export interface SampleOptions {
  count?: number; // number of candles (default 600)
  startTime?: number; // epoch ms of the first candle
  intervalMs?: number; // ms between candles (default 1h)
  startPrice?: number;
  seed?: number; // same seed -> same series
}

/** mulberry32 PRNG — small, fast, deterministic. Also reused by the Random
 *  Entry benchmark (BENCH-002) so the workspace has exactly one PRNG. */
export function mulberry32(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function makeSampleCandles(opts: SampleOptions = {}): DbCandle[] {
  const count = opts.count ?? 600;
  const startTime = opts.startTime ?? Date.UTC(2024, 0, 1);
  const intervalMs = opts.intervalMs ?? 3_600_000;
  const rand = mulberry32(opts.seed ?? 42);

  let price = opts.startPrice ?? 100;
  const out: DbCandle[] = [];
  for (let i = 0; i < count; i++) {
    const drift = (rand() - 0.48) * 0.02; // slight upward bias
    const open = price;
    const close = Math.max(0.01, open * (1 + drift));
    const high = Math.max(open, close) * (1 + rand() * 0.008);
    const low = Math.min(open, close) * (1 - rand() * 0.008);
    const volume = 100 + rand() * 200;
    out.push({ timestamp: startTime + i * intervalMs, open, high, low, close, volume });
    price = close;
  }
  return out;
}
