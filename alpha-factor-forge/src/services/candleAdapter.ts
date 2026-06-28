// Map persisted candles (snake-ish field names) to the core engine's short
// shape. The DB/bridge Candle uses timestamp/open/high/low/close/volume; the
// core backtest engine uses t/o/h/l/c/v. Keep this conversion in one place.

import type { Candle as DbCandle } from '../tauri-client/commands';
import type { Candle as CoreCandle } from '../core/backtest';

export function toCoreCandle(c: DbCandle): CoreCandle {
  return { t: c.timestamp, o: c.open, h: c.high, l: c.low, c: c.close, v: c.volume };
}

export function toCoreCandles(cs: DbCandle[]): CoreCandle[] {
  return cs.map(toCoreCandle);
}
