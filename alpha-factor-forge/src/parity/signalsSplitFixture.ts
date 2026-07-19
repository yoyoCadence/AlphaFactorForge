// TS-reference builder for the RS-CORE-003 parity fixture: params-mode
// signals, the VAL-001 validation split, and the VAL-003 embargo derivation.
// Pure and deterministic; scripts/generate-signals-split-fixtures.ts owns IO.
//
// Per the PR #66 Resolution D2, v1 discovery candidates are params-mode only,
// so ONLY the params signal path and the params embargo derivation are ported
// to Rust; blocks/code stay TypeScript-only. Every error expectation is HELD
// by the TypeScript reference (PR #69 review): generation executes the real
// TS function and asserts the throw + fragment before anything is committed.

import { planValidationSplit } from '../core/validation/split';
import { buildParamsSignals } from '../services/strategySignals';
import { deriveEmbargoBars } from '../services/embargo';
import { defaultStrategy, type ParamsStrategy, type SignalId } from '../services/strategy';
import { makeSampleCandles } from '../services/sampleData';
import type { Candle as CoreCandle } from '../core/backtest';
import { FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';

export const SIGNALS_SPLIT_PARITY_FIXTURE_VERSION = 'signals-split-parity-v1';
export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const CANDLE_CONTRACT_VERSION = 'ohlcv-candle-v1';
export const PARAMS_SIGNALS_CONTRACT_VERSION = 'params-signals-v1';
/** VAL-001 docs/validation-split-contract.md. */
export const SPLIT_CONTRACT_VERSION = 'validation-split-v1';
/** VAL-003 embargo derivation section of the same contract doc. */
export const EMBARGO_CONTRACT_VERSION = 'embargo-derivation-v1';

export interface FixtureCandle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

const toCore = (candles: FixtureCandle[]): CoreCandle[] =>
  candles.map((candle) => ({
    t: candle.timestamp,
    o: candle.open,
    h: candle.high,
    l: candle.low,
    c: candle.close,
    v: candle.volume,
  }));

const HOUR = 3_600_000;
const T0 = Date.UTC(2024, 0, 1);

/** Flat candles from a close series (indicator inputs only use closes here,
 *  matching the services test convention). */
const flat = (closes: number[]): FixtureCandle[] =>
  closes.map((close, index) => ({
    timestamp: T0 + index * HOUR,
    open: close,
    high: close,
    low: close,
    close,
    volume: 1,
  }));

/** The strategy fields the Rust params port reads. Blocks/code fields are
 *  deliberately excluded from the fixture (TypeScript-only per D2). */
export interface ParamsSignalConfig {
  fastMA: number;
  slowMA: number;
  emaPeriod: number;
  rsiPeriod: number;
  rsiBuy: number;
  rsiSell: number;
  macdFast: number;
  macdSlow: number;
  macdSignal: number;
  bbPeriod: number;
  bbMult: number;
  entrySig: SignalId;
  exitSig: SignalId;
}

const toStrategy = (config: ParamsSignalConfig): ParamsStrategy => ({
  ...defaultStrategy(),
  ...config,
  mode: 'params',
});

export const paramsConfig = (over: Partial<ParamsSignalConfig> = {}): ParamsSignalConfig => {
  const base = defaultStrategy();
  return {
    fastMA: base.fastMA,
    slowMA: base.slowMA,
    emaPeriod: base.emaPeriod,
    rsiPeriod: base.rsiPeriod,
    rsiBuy: base.rsiBuy,
    rsiSell: base.rsiSell,
    macdFast: base.macdFast,
    macdSlow: base.macdSlow,
    macdSignal: base.macdSignal,
    bbPeriod: base.bbPeriod,
    bbMult: base.bbMult,
    entrySig: base.entrySig,
    exitSig: base.exitSig,
    ...over,
  };
};

interface SignalCaseDefinition {
  id: string;
  candles: FixtureCandle[];
  config: ParamsSignalConfig;
  /** Generation-time invariant so a scenario cannot silently degenerate. */
  sanity: (entry: boolean[], exit: boolean[]) => void;
}

const requireFires = (id: string, series: boolean[], label: string): void => {
  if (!series.some(Boolean)) throw new Error(`${id}: ${label} signals never fire`);
};

function buildSignalCases(): SignalCaseDefinition[] {
  const sample = makeSampleCandles({
    count: 80,
    startTime: T0,
    intervalMs: HOUR,
    startPrice: 100,
    seed: 42,
  }) as FixtureCandle[];

  const sampleCase = (
    id: string,
    over: Partial<ParamsSignalConfig>,
  ): SignalCaseDefinition => ({
    id,
    candles: sample,
    config: paramsConfig(over),
    sanity: (entry, exit) => {
      requireFires(id, entry, 'entry');
      requireFires(id, exit, 'exit');
      if (entry[0] || exit[0]) throw new Error(`${id}: bar 0 must never signal`);
    },
  });

  return [
    {
      // Hand-verified cross (mirrors the services unit test): sma2/sma3 over
      // this series cross upward exactly at index 4.
      id: 'hand-ma-cross-exact-index',
      candles: flat([10, 8, 6, 8, 10, 12]),
      config: paramsConfig({ fastMA: 2, slowMA: 3 }),
      sanity: (entry, exit) => {
        const expected = [false, false, false, false, true, false];
        if (entry.join() !== expected.join()) {
          throw new Error('hand-ma-cross-exact-index: entry must fire only at bar 4');
        }
        if (exit.some(Boolean)) {
          throw new Error('hand-ma-cross-exact-index: no downward cross exists');
        }
      },
    },
    sampleCase('sample-ma-cross', { fastMA: 5, slowMA: 12, entrySig: 'maCrossUp', exitSig: 'maCrossDown' }),
    sampleCase('sample-ema-cross', { emaPeriod: 9, entrySig: 'emaCrossUp', exitSig: 'emaCrossDown' }),
    sampleCase('sample-price-vs-slow', { slowMA: 10, entrySig: 'priceAboveSlow', exitSig: 'priceBelowSlow' }),
    sampleCase('sample-rsi-thresholds', { rsiPeriod: 7, rsiBuy: 45, rsiSell: 55, entrySig: 'rsiOversold', exitSig: 'rsiOverbought' }),
    sampleCase('sample-macd-cross', { macdFast: 5, macdSlow: 13, macdSignal: 4, entrySig: 'macdCrossUp', exitSig: 'macdCrossDown' }),
    sampleCase('sample-bollinger-touch', { bbPeriod: 12, bbMult: 1.25, entrySig: 'bbLowerTouch', exitSig: 'bbUpperTouch' }),
  ];
}

interface SplitCaseDefinition {
  id: string;
  totalBars: number;
  embargoBars: number;
}

/** Residues 0..4 of the usable-bar count are all represented, plus zero and
 *  non-zero embargo and the safe-integer extreme from the VAL-001 tests. */
function buildSplitCases(): SplitCaseDefinition[] {
  return [
    { id: 'split-minimal-five', totalBars: 5, embargoBars: 0 },
    { id: 'split-residue-0', totalBars: 10, embargoBars: 0 },
    { id: 'split-residue-1', totalBars: 11, embargoBars: 0 },
    { id: 'split-residue-2', totalBars: 12, embargoBars: 0 },
    { id: 'split-residue-3', totalBars: 13, embargoBars: 0 },
    { id: 'split-residue-4', totalBars: 14, embargoBars: 0 },
    { id: 'split-with-embargo', totalBars: 100, embargoBars: 22 },
    { id: 'split-embargo-residue', totalBars: 97, embargoBars: 7 },
    { id: 'split-max-safe-integer', totalBars: 9_007_199_254_740_991, embargoBars: 0 },
  ];
}

interface EmbargoCaseDefinition {
  id: string;
  config: ParamsSignalConfig;
  holdingAllowanceBars: number;
}

const MAX_SAFE = Number.MAX_SAFE_INTEGER;

function buildEmbargoCases(): EmbargoCaseDefinition[] {
  return [
    { id: 'embargo-default-ma-cross', config: paramsConfig(), holdingAllowanceBars: 0 },
    { id: 'embargo-macd-exit', config: paramsConfig({ exitSig: 'macdCrossDown' }), holdingAllowanceBars: 0 },
    { id: 'embargo-rsi-pair', config: paramsConfig({ entrySig: 'rsiOversold', exitSig: 'rsiOverbought' }), holdingAllowanceBars: 0 },
    { id: 'embargo-bollinger-pair', config: paramsConfig({ entrySig: 'bbLowerTouch', exitSig: 'bbUpperTouch' }), holdingAllowanceBars: 0 },
    { id: 'embargo-price-vs-slow', config: paramsConfig({ entrySig: 'priceAboveSlow', exitSig: 'priceBelowSlow' }), holdingAllowanceBars: 0 },
    { id: 'embargo-with-allowance', config: paramsConfig(), holdingAllowanceBars: 8 },
    { id: 'embargo-unused-period-ignored', config: paramsConfig({ emaPeriod: 500 }), holdingAllowanceBars: 0 },
    // PR #70 review: embargoBars lands EXACTLY on Number.MAX_SAFE_INTEGER
    // (default MA-cross lookback 22 + this allowance) and must stay exact in
    // both languages.
    { id: 'embargo-exact-safe-boundary', config: paramsConfig(), holdingAllowanceBars: MAX_SAFE - 22 },
  ];
}

interface ErrorExpectation {
  id: string;
  expectedErrorIncludes: string;
}

/** Execute `run` against the TS reference and require the recorded failure. */
function heldError(run: () => void, expectation: ErrorExpectation): ErrorExpectation {
  let thrown: unknown = null;
  try {
    run();
  } catch (error) {
    thrown = error;
  }
  if (thrown === null) {
    throw new Error(`${expectation.id}: the TS reference did not throw`);
  }
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  if (!message.includes(expectation.expectedErrorIncludes)) {
    throw new Error(
      `${expectation.id}: TS error "${message}" must mention ${expectation.expectedErrorIncludes}`,
    );
  }
  return expectation;
}

export interface FixtureSourceHashes {
  generator: string;
  strategySignals: string;
  split: string;
  embargo: string;
  strategy: string;
  indicators: string;
  sampleData: string;
}

export function buildSignalsSplitParityFixture(sourceHashes: FixtureSourceHashes) {
  const signalCases = buildSignalCases().map((definition) => {
    const { entry, exit } = buildParamsSignals(toCore(definition.candles), toStrategy(definition.config));
    definition.sanity(entry, exit);
    return {
      id: definition.id,
      input: { candles: definition.candles, config: definition.config },
      expected: { entry, exit },
    };
  });

  const splitCases = buildSplitCases().map((definition) => ({
    id: definition.id,
    input: { totalBars: definition.totalBars, embargoBars: definition.embargoBars },
    expected: planValidationSplit(definition.totalBars, definition.embargoBars),
  }));

  const embargoCases = buildEmbargoCases().map((definition) => ({
    id: definition.id,
    input: {
      config: definition.config,
      holdingAllowanceBars: definition.holdingAllowanceBars,
    },
    expected: deriveEmbargoBars(toStrategy(definition.config), definition.holdingAllowanceBars),
  }));

  const stochConfig = paramsConfig({ entrySig: 'stochOversold' });
  const signalErrorCases = [
    {
      ...heldError(
        () => buildParamsSignals(toCore(flat([1, 2, 3, 4, 5])), toStrategy(stochConfig)),
        { id: 'signals-stoch-unsupported', expectedErrorIncludes: 'stoch' },
      ),
      input: { candles: flat([1, 2, 3, 4, 5]), config: stochConfig },
    },
  ];

  const splitErrorInputs = [
    { id: 'split-too-few-usable-bars', totalBars: 4, embargoBars: 0, fragment: 'usable bars' },
    { id: 'split-embargo-consumes-bars', totalBars: 10, embargoBars: 3, fragment: 'usable bars' },
    { id: 'split-negative-total', totalBars: -1, embargoBars: 0, fragment: 'totalBars' },
    { id: 'split-negative-embargo', totalBars: 10, embargoBars: -2, fragment: 'embargoBars' },
  ];
  const splitErrorCases = splitErrorInputs.map((definition) => ({
    ...heldError(() => planValidationSplit(definition.totalBars, definition.embargoBars), {
      id: definition.id,
      expectedErrorIncludes: definition.fragment,
    }),
    input: { totalBars: definition.totalBars, embargoBars: definition.embargoBars },
  }));

  const embargoErrorInputs: { id: string; config: ParamsSignalConfig; allowance: number; fragment: string }[] = [
    { id: 'embargo-stoch-unsupported', config: paramsConfig({ entrySig: 'stochOverbought' }), allowance: 0, fragment: 'stoch' },
    { id: 'embargo-invalid-used-period', config: paramsConfig({ fastMA: 0 }), allowance: 0, fragment: 'fastMA' },
    { id: 'embargo-negative-allowance', config: paramsConfig(), allowance: -1, fragment: 'holdingAllowanceBars' },
    // PR #70 review safe-integer boundaries: a raw period past MAX_SAFE, a
    // legal period whose DERIVED lookback leaves the safe range (IEEE-754
    // would silently round where i64 would not), and an allowance sum that
    // overflows the final embargoBars.
    {
      id: 'embargo-period-above-safe-range',
      config: paramsConfig({ entrySig: 'rsiOversold', exitSig: 'rsiOverbought', rsiPeriod: MAX_SAFE + 1 }),
      allowance: 0,
      fragment: 'rsiPeriod must be a positive integer',
    },
    {
      id: 'embargo-derived-lookback-overflow',
      config: paramsConfig({ entrySig: 'rsiOversold', exitSig: 'rsiOverbought', rsiPeriod: MAX_SAFE }),
      allowance: 0,
      fragment: 'derived signal lookback exceeds the safe integer range',
    },
    {
      id: 'embargo-allowance-overflow',
      config: paramsConfig(),
      allowance: MAX_SAFE,
      fragment: 'derived embargoBars exceeds the safe integer range',
    },
  ];
  const embargoErrorCases = embargoErrorInputs.map((definition) => ({
    ...heldError(
      () => deriveEmbargoBars(toStrategy(definition.config), definition.allowance),
      { id: definition.id, expectedErrorIncludes: definition.fragment },
    ),
    input: { config: definition.config, holdingAllowanceBars: definition.allowance },
  }));

  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: SIGNALS_SPLIT_PARITY_FIXTURE_VERSION,
    contracts: {
      candle: CANDLE_CONTRACT_VERSION,
      signals: PARAMS_SIGNALS_CONTRACT_VERSION,
      split: SPLIT_CONTRACT_VERSION,
      embargo: EMBARGO_CONTRACT_VERSION,
    },
    generator: {
      command: 'npm run fixtures:signals-split',
      referenceRuntime: 'typescript',
      sourceHashEncoding: FIXTURE_SOURCE_HASH_ENCODING,
      sourceHashes,
    },
    tolerance: {
      // Every EXPECTED OUTPUT leaf is exact: booleans, integer bar
      // ranges/counts, and integer lookbacks. Inputs still carry floats
      // (OHLCV values, RSI thresholds, bbMult) — exactness is a property of
      // the outputs, not the whole envelope.
      exact: [
        'schemaVersion and contract versions',
        'case ids and inputs',
        'entry/exit boolean arrays, positions and lengths',
        'split ranges, counts, and embargo gaps',
        'embargo lookback/allowance breakdowns',
        'error-case messages contain their expected fragment',
      ],
    },
    signalCases,
    splitCases,
    embargoCases,
    signalErrorCases,
    splitErrorCases,
    embargoErrorCases,
  };
}

export type SignalsSplitParityFixture = ReturnType<typeof buildSignalsSplitParityFixture>;
