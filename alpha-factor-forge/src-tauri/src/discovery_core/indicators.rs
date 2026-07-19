//! `indicator-v1`: pure Rust parity port of `src/core/indicators`.
//!
//! Every output stays aligned to the input length and uses `NaN` for warm-up
//! positions, matching TypeScript. ATR intentionally uses the same SMA-seeded
//! EMA smoothing as the current TS contract; changing to RMA requires a
//! reviewed contract-version bump and regenerated fixtures.
//! Candle values are finite and periods are positive under the upstream input
//! contract. Zero periods return an aligned `NaN` series rather than producing
//! a usable discovery result.

use std::fmt;

pub const INDICATOR_CONTRACT_VERSION: &str = "indicator-v1";
pub type Series = Vec<f64>;

fn nan_series(length: usize) -> Series {
    vec![f64::NAN; length]
}

pub fn sma(values: &[f64], period: usize) -> Series {
    let mut output = nan_series(values.len());
    if period == 0 {
        return output;
    }
    let mut sum = 0.0;
    for index in 0..values.len() {
        sum += values[index];
        if index >= period {
            sum -= values[index - period];
        }
        if index >= period - 1 {
            output[index] = sum / period as f64;
        }
    }
    output
}

pub fn ema(values: &[f64], period: usize) -> Series {
    let mut output = nan_series(values.len());
    if period == 0 || values.len() < period {
        return output;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut previous = 0.0;
    for value in values.iter().take(period) {
        previous += value;
    }
    previous /= period as f64;
    output[period - 1] = previous;
    for index in period..values.len() {
        previous = values[index] * multiplier + previous * (1.0 - multiplier);
        output[index] = previous;
    }
    output
}

pub fn wma(values: &[f64], period: usize) -> Series {
    let mut output = nan_series(values.len());
    if period == 0 {
        return output;
    }
    let denominator = (period * (period + 1)) as f64 / 2.0;
    for index in (period - 1)..values.len() {
        let mut accumulator = 0.0;
        for offset in 0..period {
            accumulator += values[index - offset] * (period - offset) as f64;
        }
        output[index] = accumulator / denominator;
    }
    output
}

pub fn rsi(values: &[f64], period: usize) -> Series {
    let mut output = nan_series(values.len());
    if period == 0 || values.len() <= period {
        return output;
    }
    let mut gain = 0.0;
    let mut loss = 0.0;
    for index in 1..=period {
        let change = values[index] - values[index - 1];
        if change >= 0.0 {
            gain += change;
        } else {
            loss -= change;
        }
    }
    gain /= period as f64;
    loss /= period as f64;
    output[period] = if loss == 0.0 {
        100.0
    } else {
        100.0 - 100.0 / (1.0 + gain / loss)
    };
    for index in (period + 1)..values.len() {
        let change = values[index] - values[index - 1];
        let current_gain = if change > 0.0 { change } else { 0.0 };
        let current_loss = if change < 0.0 { -change } else { 0.0 };
        gain = (gain * (period - 1) as f64 + current_gain) / period as f64;
        loss = (loss * (period - 1) as f64 + current_loss) / period as f64;
        output[index] = if loss == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + gain / loss)
        };
    }
    output
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacdOutput {
    pub macd: Series,
    pub signal: Series,
    pub hist: Series,
}

pub fn macd(values: &[f64], fast: usize, slow: usize, signal_period: usize) -> MacdOutput {
    let fast_ema = ema(values, fast);
    let slow_ema = ema(values, slow);
    let macd_line: Series = fast_ema
        .iter()
        .zip(&slow_ema)
        .map(|(fast_value, slow_value)| {
            if fast_value.is_finite() && slow_value.is_finite() {
                fast_value - slow_value
            } else {
                f64::NAN
            }
        })
        .collect();
    let first_valid = macd_line.iter().position(|value| value.is_finite());
    let mut signal = nan_series(values.len());
    if let Some(first_valid) = first_valid {
        let defined: Series = macd_line[first_valid..]
            .iter()
            .map(|value| if value.is_finite() { *value } else { 0.0 })
            .collect();
        let signal_tail = ema(&defined, signal_period);
        for (offset, value) in signal_tail.into_iter().enumerate() {
            signal[first_valid + offset] = value;
        }
    }
    let hist = macd_line
        .iter()
        .zip(&signal)
        .map(|(macd_value, signal_value)| {
            if macd_value.is_finite() && signal_value.is_finite() {
                macd_value - signal_value
            } else {
                f64::NAN
            }
        })
        .collect();
    MacdOutput {
        macd: macd_line,
        signal,
        hist,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndicatorError {
    message: String,
}

impl IndicatorError {
    fn length_mismatch(high: usize, low: usize, close: usize) -> Self {
        Self {
            message: format!(
                "OHLC series lengths must match: high={high}, low={low}, close={close}"
            ),
        }
    }
}

impl fmt::Display for IndicatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for IndicatorError {}

pub fn true_range(high: &[f64], low: &[f64], close: &[f64]) -> Result<Series, IndicatorError> {
    if high.len() != low.len() || high.len() != close.len() {
        return Err(IndicatorError::length_mismatch(
            high.len(),
            low.len(),
            close.len(),
        ));
    }
    let mut output = nan_series(high.len());
    for index in 0..high.len() {
        output[index] = if index == 0 {
            high[index] - low[index]
        } else {
            (high[index] - low[index])
                .max((high[index] - close[index - 1]).abs())
                .max((low[index] - close[index - 1]).abs())
        };
    }
    Ok(output)
}

pub fn atr(
    high: &[f64],
    low: &[f64],
    close: &[f64],
    period: usize,
) -> Result<Series, IndicatorError> {
    Ok(ema(&true_range(high, low, close)?, period))
}

#[derive(Clone, Debug, PartialEq)]
pub struct BollingerBandsOutput {
    pub middle: Series,
    pub upper: Series,
    pub lower: Series,
}

pub fn bbands(values: &[f64], period: usize, multiplier: f64) -> BollingerBandsOutput {
    let middle = sma(values, period);
    let mut upper = nan_series(values.len());
    let mut lower = nan_series(values.len());
    if period == 0 {
        return BollingerBandsOutput {
            middle,
            upper,
            lower,
        };
    }
    for index in (period - 1)..values.len() {
        let mut accumulator = 0.0;
        for offset in 0..period {
            let delta = values[index - offset] - middle[index];
            accumulator += delta * delta;
        }
        let deviation = (accumulator / period as f64).sqrt();
        upper[index] = middle[index] + multiplier * deviation;
        lower[index] = middle[index] - multiplier * deviation;
    }
    BollingerBandsOutput {
        middle,
        upper,
        lower,
    }
}

pub fn stddev(values: &[f64], period: usize) -> Series {
    let mean = sma(values, period);
    let mut output = nan_series(values.len());
    if period == 0 {
        return output;
    }
    for index in (period - 1)..values.len() {
        let mut accumulator = 0.0;
        for offset in 0..period {
            let delta = values[index - offset] - mean[index];
            accumulator += delta * delta;
        }
        output[index] = (accumulator / period as f64).sqrt();
    }
    output
}

pub fn highest(values: &[f64], period: usize) -> Series {
    let mut output = nan_series(values.len());
    if period == 0 {
        return output;
    }
    for index in (period - 1)..values.len() {
        let mut maximum = f64::NEG_INFINITY;
        for offset in 0..period {
            maximum = maximum.max(values[index - offset]);
        }
        output[index] = maximum;
    }
    output
}

pub fn lowest(values: &[f64], period: usize) -> Series {
    let mut output = nan_series(values.len());
    if period == 0 {
        return output;
    }
    for index in (period - 1)..values.len() {
        let mut minimum = f64::INFINITY;
        for offset in 0..period {
            minimum = minimum.min(values[index - offset]);
        }
        output[index] = minimum;
    }
    output
}

pub fn roc(values: &[f64], period: usize) -> Series {
    let mut output = nan_series(values.len());
    if period == 0 {
        return output;
    }
    for index in period..values.len() {
        let base = values[index - period];
        output[index] = if base == 0.0 {
            f64::NAN
        } else {
            ((values[index] - base) / base) * 100.0
        };
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_averages_preserve_alignment_and_seed_contracts() {
        let values = [1.0, 2.0, 3.0, 4.0];
        let simple = sma(&values, 3);
        assert!(simple[0].is_nan());
        assert!(simple[1].is_nan());
        assert_eq!(simple[2], 2.0);
        assert_eq!(simple[3], 3.0);

        let exponential = ema(&values, 2);
        assert!(exponential[0].is_nan());
        assert_eq!(exponential[1], 1.5);
        assert_eq!(exponential[2], 2.5);

        assert!(sma(&values, 0).iter().all(|value| value.is_nan()));
        assert!(wma(&values, 0).iter().all(|value| value.is_nan()));
    }

    #[test]
    fn rsi_and_roc_match_reference_edge_semantics() {
        let rising: Vec<f64> = (1..=16).map(|value| value as f64).collect();
        assert_eq!(rsi(&rising, 14)[15], 100.0);
        let rate = roc(&[0.0, 10.0, 11.0], 1);
        assert!(rate[1].is_nan());
        assert_eq!(rate[2], 10.0);
    }

    #[test]
    fn ohlc_inputs_fail_closed_when_lengths_differ() {
        let error = true_range(&[2.0], &[1.0, 1.5], &[1.5]).unwrap_err();
        assert!(error.to_string().contains("lengths must match"));
    }
}
