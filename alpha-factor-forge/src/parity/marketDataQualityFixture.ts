// TypeScript-reference builder for the DATA-QUALITY-001 market-data admission
// parity fixture. Pure and deterministic;
// scripts/generate-market-data-quality-fixtures.ts owns file IO.
//
// This module is DELIBERATELY independent of src/core/market-data/quality.ts.
// Every expectation below is authored from the adjudicated specification, not
// recorded from a running validator, so the fixture is a contract both runtimes
// are measured against rather than a snapshot of whichever one was written
// first. Nothing here may import the validator.

import { FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';

export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const MARKET_DATA_QUALITY_FIXTURE_VERSION = 'market-data-quality-parity-v1';
export const MARKET_DATA_QUALITY_CONTRACT_VERSION = 'market-data-quality-v1';
export const CANDLE_CONTRACT_VERSION = 'ohlcv-candle-v1';
export const SPECIAL_INPUT_NUMBER_ENCODING = 'explicit-numeric-status-v1';
export const EXPECTED_NUMERIC_POLICY = 'exact-v1';

/** Adjudicated product plausibility boundary — planning decision 1. */
export const MIN_MARKET_TIMESTAMP_MS = 946_684_800_000;
export const MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE = 4_102_444_800_000;

/** Stable rule ids in evaluation order. */
export const MARKET_DATA_RULE_IDS = [
  'timestamp_not_integer',
  'timestamp_out_of_range',
  'timestamp_not_representable',
  'price_not_positive',
  'volume_negative',
  'high_below_low',
  'ohlc_out_of_range',
] as const;

export type FixtureRuleId = (typeof MARKET_DATA_RULE_IDS)[number];

/**
 * `timestamp_not_representable` is unreachable by construction: the product
 * range is strictly inside both JavaScript's Date limit and chrono's UTC
 * millisecond range, so nothing that survives rule 2 can fail rule 3. It is
 * therefore covered by a per-language predicate unit test instead of a shared
 * fixture row — the two runtimes disagree about which values are representable
 * OUTSIDE the product range, which is exactly why that check cannot be shared.
 */
export const REACHABLE_RULE_IDS: readonly FixtureRuleId[] = MARKET_DATA_RULE_IDS.filter(
  (rule) => rule !== 'timestamp_not_representable',
);

/** Non-finite inputs cannot be written as JSON numbers, so they travel as tags. */
export type FixtureNumericTag = 'nan' | 'positive_infinity' | 'negative_infinity';
export type FixtureNumber = number | FixtureNumericTag;

export interface FixtureCandle {
  timestamp: FixtureNumber;
  open: FixtureNumber;
  high: FixtureNumber;
  low: FixtureNumber;
  close: FixtureNumber;
  volume: FixtureNumber;
}

export type FixtureExpectation =
  | { accepted: true }
  | { accepted: false; rule: FixtureRuleId; index: number };

export interface MarketDataQualityCase {
  id: string;
  candles: FixtureCandle[];
  expected: FixtureExpectation;
}

export function decodeFixtureNumber(value: FixtureNumber): number {
  if (typeof value === 'number') return value;
  switch (value) {
    case 'nan':
      return NaN;
    case 'positive_infinity':
      return Infinity;
    case 'negative_infinity':
      return -Infinity;
  }
}

// ---------- authored inputs ----------

const T0 = 1_721_001_600_000; // 2024-07-15T00:00:00Z
const HOUR = 3_600_000;

function baseCandles(): FixtureCandle[] {
  return [
    { timestamp: T0, open: 100, high: 103, low: 99, close: 102, volume: 10 },
    { timestamp: T0 + HOUR, open: 102, high: 105, low: 101, close: 104, volume: 12 },
    { timestamp: T0 + 2 * HOUR, open: 104, high: 106, low: 103, close: 105, volume: 8 },
  ];
}

function mutate(index: number, patch: Partial<FixtureCandle>): FixtureCandle[] {
  const candles = baseCandles();
  candles[index] = { ...candles[index], ...patch };
  return candles;
}

function singleCandle(patch: Partial<FixtureCandle>): FixtureCandle[] {
  return [{ timestamp: T0, open: 100, high: 103, low: 99, close: 102, volume: 10, ...patch }];
}

function accept(id: string, candles: FixtureCandle[]): MarketDataQualityCase {
  return { id, candles, expected: { accepted: true } };
}

function reject(
  id: string,
  candles: FixtureCandle[],
  rule: FixtureRuleId,
  index: number,
): MarketDataQualityCase {
  return { id, candles, expected: { accepted: false, rule, index } };
}

function buildCases(): MarketDataQualityCase[] {
  return [
    // ---- accepted ----
    accept('valid-three-candle-dataset', baseCandles()),
    // Volume is NON-negative, not strictly positive: a zero-volume bar is real data.
    accept('valid-single-candle-zero-volume', singleCandle({ volume: 0 })),
    // A flat bar where open == high == low == close is admissible.
    accept(
      'valid-flat-bar',
      singleCandle({ open: 100, high: 100, low: 100, close: 100, volume: 1 }),
    ),

    // ---- boundary rows (inclusive lower, exclusive upper) ----
    accept('boundary-min-timestamp-accepted', singleCandle({ timestamp: MIN_MARKET_TIMESTAMP_MS })),
    accept(
      'boundary-max-exclusive-minus-one-accepted',
      singleCandle({ timestamp: MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE - 1 }),
    ),
    reject(
      'boundary-min-minus-one-rejected',
      singleCandle({ timestamp: MIN_MARKET_TIMESTAMP_MS - 1 }),
      'timestamp_out_of_range',
      0,
    ),
    reject(
      'boundary-max-exclusive-rejected',
      singleCandle({ timestamp: MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE }),
      'timestamp_out_of_range',
      0,
    ),

    // ---- the two named audit values ----
    // Epoch SECONDS for 2024-01-01, silently read as milliseconds => 1970-01-20.
    reject(
      'audit-epoch-seconds-timestamp-rejected',
      mutate(0, { timestamp: 1_704_067_200 }),
      'timestamp_out_of_range',
      0,
    ),
    // The value the runner's chrono regression uses: a JavaScript-safe integer
    // that is nevertheless implausible market data. Rule 2 precedes rule 3, so
    // both runtimes must report out-of-range rather than not-representable.
    reject(
      'runner-regression-js-safe-timestamp-rejected',
      mutate(2, { timestamp: 8_500_000_000_000_000 }),
      'timestamp_out_of_range',
      2,
    ),

    // ---- rule 1: timestamp_not_integer ----
    reject(
      'timestamp-fractional-rejected',
      mutate(1, { timestamp: T0 + HOUR + 0.5 }),
      'timestamp_not_integer',
      1,
    ),
    // 2^53 is the first integer JavaScript can no longer represent safely.
    reject(
      'timestamp-unsafe-magnitude-rejected',
      mutate(1, { timestamp: 9_007_199_254_740_992 }),
      'timestamp_not_integer',
      1,
    ),
    reject(
      'timestamp-nan-rejected',
      mutate(0, { timestamp: 'nan' }),
      'timestamp_not_integer',
      0,
    ),
    reject(
      'timestamp-infinite-rejected',
      mutate(1, { timestamp: 'positive_infinity' }),
      'timestamp_not_integer',
      1,
    ),

    // ---- rule 4: price_not_positive ----
    reject('price-zero-low-rejected', mutate(1, { low: 0 }), 'price_not_positive', 1),
    reject(
      'price-negative-open-rejected',
      mutate(0, { open: -100, low: -101 }),
      'price_not_positive',
      0,
    ),
    reject('price-nan-high-rejected', mutate(2, { high: 'nan' }), 'price_not_positive', 2),

    // ---- rule 5: volume_negative ----
    reject('volume-negative-rejected', mutate(2, { volume: -1 }), 'volume_negative', 2),
    reject('volume-nan-rejected', mutate(0, { volume: 'nan' }), 'volume_negative', 0),

    // ---- rule 6: high_below_low ----
    reject(
      'high-below-low-rejected',
      mutate(0, { open: 100, high: 99, low: 100, close: 100 }),
      'high_below_low',
      0,
    ),

    // ---- rule 7: ohlc_out_of_range ----
    reject('ohlc-open-above-high-rejected', mutate(1, { open: 106 }), 'ohlc_out_of_range', 1),
    reject('ohlc-close-below-low-rejected', mutate(2, { close: 102 }), 'ohlc_out_of_range', 2),

    // ---- ordering proofs ----
    // The audit's own example. Every price is positive, so rule 4 passes and the
    // FIRST failing rule in evaluation order is volume_negative — not the
    // inverted high/low that a reader notices first.
    reject(
      'audit-inverted-ohlc-negative-volume-reports-volume-first',
      singleCandle({ open: 100, high: 90, low: 110, close: 100, volume: -1 }),
      'volume_negative',
      0,
    ),
    // Evaluation stops at the FIRST failing candle, so the later rule 6 defect
    // is never reported.
    reject(
      'first-failing-candle-wins-over-later-defect',
      [
        { timestamp: T0, open: 100, high: 103, low: 99, close: 102, volume: -1 },
        { timestamp: T0 + HOUR, open: 102, high: 100, low: 101, close: 102, volume: 12 },
      ],
      'volume_negative',
      0,
    ),
  ];
}

// ---------- build-time self-checks (independent of any validator) ----------

function assertMatrixCoverage(cases: MarketDataQualityCase[]): void {
  const ids = cases.map((entry) => entry.id);
  if (new Set(ids).size !== ids.length) {
    throw new Error('market-data quality fixture has duplicate case ids');
  }
  if (!cases.some((entry) => entry.expected.accepted && entry.candles.length > 1)) {
    throw new Error('market-data quality fixture needs one fully valid multi-candle dataset');
  }
  const rejectedRules = new Set(
    cases
      .filter((entry): entry is MarketDataQualityCase & { expected: { accepted: false } } =>
        !entry.expected.accepted)
      .map((entry) => (entry.expected as { rule: FixtureRuleId }).rule),
  );
  for (const rule of REACHABLE_RULE_IDS) {
    if (!rejectedRules.has(rule)) {
      throw new Error(`market-data quality fixture is missing a rejection row for ${rule}`);
    }
  }
  if (rejectedRules.has('timestamp_not_representable')) {
    throw new Error('rule 3 is unreachable and must not have a fixture rejection row');
  }
  const boundaries: [string, number, boolean][] = [
    ['boundary-min-minus-one-rejected', MIN_MARKET_TIMESTAMP_MS - 1, false],
    ['boundary-min-timestamp-accepted', MIN_MARKET_TIMESTAMP_MS, true],
    ['boundary-max-exclusive-minus-one-accepted', MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE - 1, true],
    ['boundary-max-exclusive-rejected', MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE, false],
  ];
  for (const [id, timestamp, accepted] of boundaries) {
    const found = cases.find((entry) => entry.id === id);
    if (!found) throw new Error(`missing boundary case ${id}`);
    if (found.candles.length !== 1 || found.candles[0].timestamp !== timestamp) {
      throw new Error(`boundary case ${id} must hold exactly the timestamp ${timestamp}`);
    }
    if (found.expected.accepted !== accepted) {
      throw new Error(`boundary case ${id} has the wrong expectation`);
    }
  }
  for (const entry of cases) {
    if (entry.candles.length === 0) {
      throw new Error(`case ${entry.id} must hold at least one candle`);
    }
    if (!entry.expected.accepted) {
      const { index } = entry.expected;
      if (!Number.isInteger(index) || index < 0 || index >= entry.candles.length) {
        throw new Error(`case ${entry.id} reports an out-of-bounds failing index`);
      }
    }
  }
}

export interface FixtureSourceHashes {
  generator: string;
}

export function buildMarketDataQualityParityFixture(sourceHashes: FixtureSourceHashes) {
  const cases = buildCases();
  assertMatrixCoverage(cases);
  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: MARKET_DATA_QUALITY_FIXTURE_VERSION,
    contracts: {
      marketDataQuality: MARKET_DATA_QUALITY_CONTRACT_VERSION,
      candle: CANDLE_CONTRACT_VERSION,
    },
    generator: {
      command: 'npm run fixtures:market-data-quality',
      referenceRuntime: 'typescript',
      sourceHashEncoding: FIXTURE_SOURCE_HASH_ENCODING,
      sourceHashes,
    },
    numericEncoding: {
      specialInputNumbers: SPECIAL_INPUT_NUMBER_ENCODING,
      expectedNumericPolicy: EXPECTED_NUMERIC_POLICY,
    },
    constants: {
      minTimestampMs: MIN_MARKET_TIMESTAMP_MS,
      maxTimestampMsExclusive: MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE,
    },
    ruleIds: [...MARKET_DATA_RULE_IDS],
    reachableRuleIds: [...REACHABLE_RULE_IDS],
    unreachableRuleIds: MARKET_DATA_RULE_IDS.filter((rule) => !REACHABLE_RULE_IDS.includes(rule)),
    tolerance: {
      // Admission is a classification, not a computation: every leaf compares
      // exactly. There is no tolerance to spend.
      policy: EXPECTED_NUMERIC_POLICY,
      exact: [
        'schema, fixture, and contract versions',
        'case ids and inventory order',
        'input candle field values, including the non-finite tags',
        'accepted/rejected verdicts, rule ids, and failing candle indexes',
      ],
    },
    cases,
  };
}

export type MarketDataQualityParityFixture = ReturnType<
  typeof buildMarketDataQualityParityFixture
>;
