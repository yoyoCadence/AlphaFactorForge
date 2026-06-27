// FULL — technical indicators as pure functions. No DOM/React/IO.
// Deterministic: same input array -> same output array. All return arrays
// aligned to the input length; warm-up positions are NaN.
//
// Tested in src/core/indicators/indicators.test.ts (npm test).

export type Series = number[];

const nan = Number.NaN;
const isNum = (x: number) => Number.isFinite(x);

/** Simple moving average. */
export function sma(values: Series, period: number): Series {
  const out: Series = new Array(values.length).fill(nan);
  if (period <= 0) return out;
  let sum = 0;
  for (let i = 0; i < values.length; i++) {
    sum += values[i];
    if (i >= period) sum -= values[i - period];
    if (i >= period - 1) out[i] = sum / period;
  }
  return out;
}

/** Exponential moving average. Seeded with the SMA of the first `period`. */
export function ema(values: Series, period: number): Series {
  const out: Series = new Array(values.length).fill(nan);
  if (period <= 0 || values.length < period) return out;
  const k = 2 / (period + 1);
  let prev = 0;
  for (let i = 0; i < period; i++) prev += values[i];
  prev /= period;
  out[period - 1] = prev;
  for (let i = period; i < values.length; i++) {
    prev = values[i] * k + prev * (1 - k);
    out[i] = prev;
  }
  return out;
}

/** Weighted moving average (linear weights). */
export function wma(values: Series, period: number): Series {
  const out: Series = new Array(values.length).fill(nan);
  if (period <= 0) return out;
  const denom = (period * (period + 1)) / 2;
  for (let i = period - 1; i < values.length; i++) {
    let acc = 0;
    for (let j = 0; j < period; j++) acc += values[i - j] * (period - j);
    out[i] = acc / denom;
  }
  return out;
}

/** Wilder's RSI. */
export function rsi(values: Series, period = 14): Series {
  const out: Series = new Array(values.length).fill(nan);
  if (values.length <= period) return out;
  let gain = 0;
  let loss = 0;
  for (let i = 1; i <= period; i++) {
    const ch = values[i] - values[i - 1];
    if (ch >= 0) gain += ch;
    else loss -= ch;
  }
  gain /= period;
  loss /= period;
  out[period] = loss === 0 ? 100 : 100 - 100 / (1 + gain / loss);
  for (let i = period + 1; i < values.length; i++) {
    const ch = values[i] - values[i - 1];
    const g = ch > 0 ? ch : 0;
    const l = ch < 0 ? -ch : 0;
    gain = (gain * (period - 1) + g) / period;
    loss = (loss * (period - 1) + l) / period;
    out[i] = loss === 0 ? 100 : 100 - 100 / (1 + gain / loss);
  }
  return out;
}

export interface MacdOut {
  macd: Series;
  signal: Series;
  hist: Series;
}

/** MACD line, signal line, histogram. */
export function macd(values: Series, fast = 12, slow = 26, signalPeriod = 9): MacdOut {
  const ef = ema(values, fast);
  const es = ema(values, slow);
  const macdLine = values.map((_, i) => (isNum(ef[i]) && isNum(es[i]) ? ef[i] - es[i] : nan));
  // signal = EMA of the defined portion of the macd line
  const firstValid = macdLine.findIndex(isNum);
  const signal: Series = new Array(values.length).fill(nan);
  if (firstValid >= 0) {
    const sub = macdLine.slice(firstValid).map((x) => (isNum(x) ? x : 0));
    const sig = ema(sub, signalPeriod);
    for (let i = 0; i < sig.length; i++) signal[firstValid + i] = sig[i];
  }
  const hist = macdLine.map((m, i) => (isNum(m) && isNum(signal[i]) ? m - signal[i] : nan));
  return { macd: macdLine, signal, hist };
}

/** True range series from OHLC. */
export function trueRange(high: Series, low: Series, close: Series): Series {
  const out: Series = new Array(high.length).fill(nan);
  for (let i = 0; i < high.length; i++) {
    if (i === 0) {
      out[i] = high[i] - low[i];
    } else {
      out[i] = Math.max(
        high[i] - low[i],
        Math.abs(high[i] - close[i - 1]),
        Math.abs(low[i] - close[i - 1]),
      );
    }
  }
  return out;
}

/** Average true range (Wilder smoothing). */
export function atr(high: Series, low: Series, close: Series, period = 14): Series {
  const tr = trueRange(high, low, close);
  return ema(tr, period); // EMA is an acceptable ATR smoothing; swap to RMA if desired
}

export interface BbandsOut {
  middle: Series;
  upper: Series;
  lower: Series;
}

/** Bollinger Bands. */
export function bbands(values: Series, period = 20, mult = 2): BbandsOut {
  const middle = sma(values, period);
  const upper: Series = new Array(values.length).fill(nan);
  const lower: Series = new Array(values.length).fill(nan);
  for (let i = period - 1; i < values.length; i++) {
    let acc = 0;
    for (let j = 0; j < period; j++) {
      const d = values[i - j] - middle[i];
      acc += d * d;
    }
    const sd = Math.sqrt(acc / period);
    upper[i] = middle[i] + mult * sd;
    lower[i] = middle[i] - mult * sd;
  }
  return { middle, upper, lower };
}

/** Rolling standard deviation (population). */
export function stddev(values: Series, period: number): Series {
  const mean = sma(values, period);
  const out: Series = new Array(values.length).fill(nan);
  for (let i = period - 1; i < values.length; i++) {
    let acc = 0;
    for (let j = 0; j < period; j++) {
      const d = values[i - j] - mean[i];
      acc += d * d;
    }
    out[i] = Math.sqrt(acc / period);
  }
  return out;
}

/** Highest high / lowest low over a rolling window. */
export function highest(values: Series, period: number): Series {
  const out: Series = new Array(values.length).fill(nan);
  for (let i = period - 1; i < values.length; i++) {
    let m = -Infinity;
    for (let j = 0; j < period; j++) m = Math.max(m, values[i - j]);
    out[i] = m;
  }
  return out;
}

export function lowest(values: Series, period: number): Series {
  const out: Series = new Array(values.length).fill(nan);
  for (let i = period - 1; i < values.length; i++) {
    let m = Infinity;
    for (let j = 0; j < period; j++) m = Math.min(m, values[i - j]);
    out[i] = m;
  }
  return out;
}

/** Rate of change (%). */
export function roc(values: Series, period: number): Series {
  const out: Series = new Array(values.length).fill(nan);
  for (let i = period; i < values.length; i++) {
    const base = values[i - period];
    out[i] = base === 0 ? nan : ((values[i] - base) / base) * 100;
  }
  return out;
}

// TODO(local, Phase B): ADX, STOCH, CCI, MOM, KELTNER, OBV, VOL_SMA, MFI, HLC3.
// Add each with a matching unit test before wiring into the DSL compiler.
