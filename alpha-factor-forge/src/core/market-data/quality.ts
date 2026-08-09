// DATA-QUALITY-001 — market-data admission contract (TypeScript side).
//
// Dataset identity proves only that bytes were not altered; it never proves the
// bytes describe a possible market. This module is that missing semantic gate,
// and `src-tauri/src/discovery_core/market_data.rs` is its exact Rust mirror —
// the two are held together by fixtures/rs-core/market-data-quality-v1.json.
//
// It is deliberately NOT part of the identity path: nothing here may be called
// from src/core/hashing, and the dataset hash preimage is unchanged. Admission
// is a gate, not persisted evidence.
//
// Pure module (src/core/* purity rule): no React, no DOM, no IO, and no zh-TW
// user-facing copy — callers own the wording they show a user.

export const MARKET_DATA_QUALITY_VERSION = 'market-data-quality-v1';

/**
 * Adjudicated product plausibility boundary (planning decision 1), NOT a
 * language limit: 2000-01-01T00:00:00Z inclusive to 2100-01-01T00:00:00Z
 * exclusive. The lower bound rejects epoch-seconds read as milliseconds; the
 * upper bound rejects microsecond/nanosecond values and implausible futures.
 */
export const MIN_MARKET_TIMESTAMP_MS = 946_684_800_000;
export const MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE = 4_102_444_800_000;

/** Stable rule ids, evaluated in exactly this order. */
export const MARKET_DATA_RULE_IDS = [
  'timestamp_not_integer',
  'timestamp_out_of_range',
  'timestamp_not_representable',
  'price_not_positive',
  'volume_negative',
  'high_below_low',
  'ohlc_out_of_range',
] as const;

export type MarketDataRule = (typeof MARKET_DATA_RULE_IDS)[number];

export interface MarketDataCandle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface MarketDataIssue {
  index: number;
  timestamp: number;
  rule: MarketDataRule;
}

/**
 * Rule 3's predicate, asserted independently of the range rather than left as
 * an implied consequence of it. Every value that survives rule 2 is
 * representable, so rule 3 is unreachable today — it stays as defence in depth
 * against a future range change, and is unit-tested directly.
 */
export function isRepresentableTimestamp(timestamp: number): boolean {
  return Number.isFinite(new Date(timestamp).getTime());
}

function isPositivePrice(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}

/** Classify one candle, or return null when it is admissible. */
export function inspectMarketDataCandle(
  index: number,
  candle: MarketDataCandle,
): MarketDataIssue | null {
  const { timestamp, open, high, low, close, volume } = candle;
  const at = (rule: MarketDataRule): MarketDataIssue => ({ index, timestamp, rule });

  if (!Number.isSafeInteger(timestamp)) return at('timestamp_not_integer');
  if (timestamp < MIN_MARKET_TIMESTAMP_MS || timestamp >= MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE) {
    return at('timestamp_out_of_range');
  }
  if (!isRepresentableTimestamp(timestamp)) return at('timestamp_not_representable');
  if (!isPositivePrice(open) || !isPositivePrice(high) || !isPositivePrice(low) || !isPositivePrice(close)) {
    return at('price_not_positive');
  }
  // A NaN volume fails this test, because `!(NaN >= 0)`.
  if (!(Number.isFinite(volume) && volume >= 0)) return at('volume_negative');
  if (high < low) return at('high_below_low');
  if (open < low || open > high || close < low || close > high) return at('ohlc_out_of_range');
  return null;
}

/**
 * Evaluation stops at the first failing candle, so both runtimes report the
 * same index and rule for the same input. A dataset is admitted whole or
 * rejected whole; individual bad candles are never dropped.
 */
export function firstMarketDataIssue(
  candles: readonly MarketDataCandle[],
): MarketDataIssue | null {
  for (let index = 0; index < candles.length; index++) {
    const issue = inspectMarketDataCandle(index, candles[index]);
    if (issue) return issue;
  }
  return null;
}

/** Stable technical message; UI copy is the caller's responsibility. */
export function describeMarketDataIssue(issue: MarketDataIssue): string {
  return `market data rejected at candle ${issue.index} (timestamp ${issue.timestamp}): ${issue.rule}`;
}

export function assertMarketDataQuality(candles: readonly MarketDataCandle[]): void {
  const issue = firstMarketDataIssue(candles);
  if (issue) throw new Error(describeMarketDataIssue(issue));
}
