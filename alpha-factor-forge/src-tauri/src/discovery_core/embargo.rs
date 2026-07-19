//! `embargo-derivation-v1`: pure Rust parity port of the params-mode branch
//! of `src/services/embargo.ts` (VAL-003).
//!
//! Boundary responsibility (PR #70 review): this module accepts an
//! already-validated params-only projection. `ParamsSignalConfig` carries no
//! `mode` field, so RUNNER-CONFIG MUST reject non-params candidate modes
//! before anything reaches this module; blocks/code lookback derivation stays
//! TypeScript-only per the Resolution D2. Within that contract, unsupported
//! signal ids still fail closed here with the TypeScript message.
//!
//! All derived arithmetic is bounded to the JavaScript safe-integer range so
//! both languages produce IDENTICAL integers or IDENTICAL failures — IEEE-754
//! would silently round past `Number.MAX_SAFE_INTEGER` where i64 would not.

use serde::{Deserialize, Serialize};

use super::signals::ParamsSignalConfig;

pub const EMBARGO_CONTRACT_VERSION: &str = "embargo-derivation-v1";

const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbargoDerivation {
    pub embargo_bars: i64,
    pub max_signal_lookback_bars: i64,
    pub holding_allowance_bars: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbargoError(pub String);

impl std::fmt::Display for EmbargoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EmbargoError {}

fn overflow_error(context: &str) -> EmbargoError {
    EmbargoError(format!("{context} exceeds the safe integer range"))
}

/// Raw periods must be positive AND within the JS safe-integer range before
/// any i64 conversion — `usize as i64` would wrap past `i64::MAX`, and values
/// in `(MAX_SAFE, i64::MAX]` are rejected by the TypeScript reference.
fn period(value: usize, name: &str) -> Result<i64, EmbargoError> {
    if value < 1 || value > JS_MAX_SAFE_INTEGER as usize {
        return Err(EmbargoError(format!(
            "{name} must be a positive integer to derive an embargo (got {value})"
        )));
    }
    i64::try_from(value).map_err(|_| overflow_error("derived signal lookback"))
}

/// Checked derivation step: overflow or leaving the safe range fails closed
/// with the same message as the TypeScript `safeLookback` guard.
fn checked_lookback(value: Option<i64>, context: &str) -> Result<i64, EmbargoError> {
    let value = value.ok_or_else(|| overflow_error(context))?;
    if !(0..=JS_MAX_SAFE_INTEGER).contains(&value) {
        return Err(overflow_error(context));
    }
    Ok(value)
}

fn macd_signal_lookback(config: &ParamsSignalConfig) -> Result<i64, EmbargoError> {
    let slowest = period(config.macd_fast, "macdFast")?.max(period(config.macd_slow, "macdSlow")?);
    let signal = period(config.macd_signal, "macdSignal")?;
    checked_lookback(
        slowest
            .checked_add(signal)
            .and_then(|sum| sum.checked_sub(1)),
        "derived signal lookback",
    )
}

/// History bars one params-mode signal reads (VAL-003 conventions: real
/// indicator warm-up, plus one bar for cross semantics).
fn params_signal_lookback(id: &str, config: &ParamsSignalConfig) -> Result<i64, EmbargoError> {
    const LOOKBACK: &str = "derived signal lookback";
    Ok(match id {
        "maCrossUp" | "maCrossDown" => {
            let slowest = period(config.fast_ma, "fastMA")?.max(period(config.slow_ma, "slowMA")?);
            checked_lookback(slowest.checked_add(1), LOOKBACK)?
        }
        "emaCrossUp" | "emaCrossDown" => checked_lookback(
            period(config.ema_period, "emaPeriod")?.checked_add(1),
            LOOKBACK,
        )?,
        "priceAboveSlow" | "priceBelowSlow" => period(config.slow_ma, "slowMA")?,
        "rsiOversold" | "rsiOverbought" => {
            let warmup = checked_lookback(
                period(config.rsi_period, "rsiPeriod")?.checked_add(1),
                LOOKBACK,
            )?;
            checked_lookback(warmup.checked_add(1), LOOKBACK)?
        }
        "macdCrossUp" | "macdCrossDown" => {
            checked_lookback(macd_signal_lookback(config)?.checked_add(1), LOOKBACK)?
        }
        "bbLowerTouch" | "bbUpperTouch" => period(config.bb_period, "bbPeriod")?,
        other => {
            return Err(EmbargoError(format!(
            "unsupported signal \"{other}\": stoch* signals await a core STOCH indicator (Phase B)"
        )))
        }
    })
}

/// History bars the strategy's slowest used entry/exit signal reads (>= 1).
pub fn max_signal_lookback_bars(config: &ParamsSignalConfig) -> Result<i64, EmbargoError> {
    let lookback = params_signal_lookback(&config.entry_sig, config)?
        .max(params_signal_lookback(&config.exit_sig, config)?);
    Ok(lookback.max(1))
}

/// Derive the VAL-001 `embargoBars` for a params-mode strategy. The returned
/// breakdown must be recorded alongside the run for reproducibility.
pub fn derive_embargo_bars(
    config: &ParamsSignalConfig,
    holding_allowance_bars: i64,
) -> Result<EmbargoDerivation, EmbargoError> {
    if !(0..=JS_MAX_SAFE_INTEGER).contains(&holding_allowance_bars) {
        return Err(EmbargoError(
            "holdingAllowanceBars must be a non-negative safe integer".into(),
        ));
    }
    let lookback = max_signal_lookback_bars(config)?;
    let embargo_bars = checked_lookback(
        lookback.checked_add(holding_allowance_bars),
        "derived embargoBars",
    )?;
    Ok(EmbargoDerivation {
        embargo_bars,
        max_signal_lookback_bars: lookback,
        holding_allowance_bars,
    })
}
