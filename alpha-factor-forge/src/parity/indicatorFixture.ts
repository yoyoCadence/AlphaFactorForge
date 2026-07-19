// TS-reference builder for the RS-CORE indicator parity fixture. This module
// is pure and deterministic; the scripts/ wrapper owns filesystem writes.

import {
  atr,
  bbands,
  ema,
  highest,
  lowest,
  macd,
  roc,
  rsi,
  sma,
  stddev,
  trueRange,
  wma,
  type Series,
} from '../core/indicators';
import { makeSampleCandles, type SampleOptions } from '../services/sampleData';

export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const INDICATOR_CONTRACT_VERSION = 'indicator-v1';
export const CANDLE_CONTRACT_VERSION = 'ohlcv-candle-v1';
export const SAMPLE_INPUT_CONTRACT_VERSION = 'sample-candles-v1';

export interface FixtureSourceHashes {
  generator: string;
  indicators: string;
  sampleData: string;
}

export interface EncodedIndicatorOutput {
  sma: Array<number | null>;
  ema: Array<number | null>;
  wma: Array<number | null>;
  rsi: Array<number | null>;
  macd: {
    macd: Array<number | null>;
    signal: Array<number | null>;
    hist: Array<number | null>;
  };
  trueRange: Array<number | null>;
  atr: Array<number | null>;
  bbands: {
    middle: Array<number | null>;
    upper: Array<number | null>;
    lower: Array<number | null>;
  };
  stddev: Array<number | null>;
  highest: Array<number | null>;
  lowest: Array<number | null>;
  roc: Array<number | null>;
}

export const INDICATOR_FIXTURE_OPTIONS = {
  count: 48,
  startTime: Date.UTC(2024, 0, 1),
  intervalMs: 3_600_000,
  startPrice: 100,
  seed: 42,
} as const satisfies Required<SampleOptions>;

export const INDICATOR_FIXTURE_PARAMETERS = {
  smaPeriod: 7,
  emaPeriod: 9,
  wmaPeriod: 6,
  rsiPeriod: 14,
  macdFast: 12,
  macdSlow: 26,
  macdSignal: 9,
  atrPeriod: 14,
  bbandsPeriod: 20,
  bbandsMult: 2,
  stddevPeriod: 10,
  extremaPeriod: 8,
  rocPeriod: 5,
} as const;

function encodeSeries(series: Series): Array<number | null> {
  return series.map((value) => {
    if (Number.isNaN(value)) return null;
    if (!Number.isFinite(value)) {
      throw new Error('indicator fixture cannot silently encode an infinite value');
    }
    return value;
  });
}

export function buildIndicatorParityFixture(sourceHashes: FixtureSourceHashes) {
  const candles = makeSampleCandles(INDICATOR_FIXTURE_OPTIONS);
  const close = candles.map((candle) => candle.close);
  const high = candles.map((candle) => candle.high);
  const low = candles.map((candle) => candle.low);
  const parameters = INDICATOR_FIXTURE_PARAMETERS;
  const macdOutput = macd(
    close,
    parameters.macdFast,
    parameters.macdSlow,
    parameters.macdSignal,
  );
  const bbandsOutput = bbands(close, parameters.bbandsPeriod, parameters.bbandsMult);

  const expected: EncodedIndicatorOutput = {
    sma: encodeSeries(sma(close, parameters.smaPeriod)),
    ema: encodeSeries(ema(close, parameters.emaPeriod)),
    wma: encodeSeries(wma(close, parameters.wmaPeriod)),
    rsi: encodeSeries(rsi(close, parameters.rsiPeriod)),
    macd: {
      macd: encodeSeries(macdOutput.macd),
      signal: encodeSeries(macdOutput.signal),
      hist: encodeSeries(macdOutput.hist),
    },
    trueRange: encodeSeries(trueRange(high, low, close)),
    atr: encodeSeries(atr(high, low, close, parameters.atrPeriod)),
    bbands: {
      middle: encodeSeries(bbandsOutput.middle),
      upper: encodeSeries(bbandsOutput.upper),
      lower: encodeSeries(bbandsOutput.lower),
    },
    stddev: encodeSeries(stddev(close, parameters.stddevPeriod)),
    highest: encodeSeries(highest(high, parameters.extremaPeriod)),
    lowest: encodeSeries(lowest(low, parameters.extremaPeriod)),
    roc: encodeSeries(roc(close, parameters.rocPeriod)),
  };

  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: 'indicator-parity-v1',
    contracts: {
      candle: CANDLE_CONTRACT_VERSION,
      indicators: INDICATOR_CONTRACT_VERSION,
      sampleInput: SAMPLE_INPUT_CONTRACT_VERSION,
    },
    generator: {
      command: 'npm run fixtures:indicators',
      referenceRuntime: 'typescript',
      sourceHashes,
    },
    tolerance: {
      default: { absolute: 1e-12, relative: 1e-10 },
      exact: [
        'schemaVersion and contract versions',
        'case ids and parameter integers',
        'candle timestamps and array lengths',
        'warm-up null positions',
      ],
    },
    cases: [
      {
        id: 'sample-seed-42-48-bars',
        input: {
          provenance: {
            kind: 'synthetic-fixture-input-only',
            generator: 'makeSampleCandles',
            options: INDICATOR_FIXTURE_OPTIONS,
          },
          candles,
          parameters,
        },
        expected,
      },
    ],
  };
}

export type IndicatorParityFixture = ReturnType<typeof buildIndicatorParityFixture>;
