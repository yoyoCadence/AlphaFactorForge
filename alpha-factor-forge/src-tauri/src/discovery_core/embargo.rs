//! `embargo-derivation-v1`: pure Rust parity port of the params-mode branch
//! of `src/services/embargo.ts` (VAL-003).
//!
//! Per the PR #66 Resolution D2, only params-mode candidates exist in v1
//! discovery, so blocks/code lookback derivation stays TypeScript-only and
//! any non-params usage fails closed here by construction (unsupported
//! signal ids are rejected).

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

fn period(value: usize, name: &str) -> Result<i64, EmbargoError> {
    if value < 1 {
        return Err(EmbargoError(format!(
            "{name} must be a positive integer to derive an embargo (got {value})"
        )));
    }
    Ok(value as i64)
}

fn macd_signal_lookback(config: &ParamsSignalConfig) -> Result<i64, EmbargoError> {
    Ok(
        period(config.macd_fast, "macdFast")?.max(period(config.macd_slow, "macdSlow")?)
            + period(config.macd_signal, "macdSignal")?
            - 1,
    )
}

/// History bars one params-mode signal reads (VAL-003 conventions: real
/// indicator warm-up, plus one bar for cross semantics).
fn params_signal_lookback(id: &str, config: &ParamsSignalConfig) -> Result<i64, EmbargoError> {
    Ok(match id {
        "maCrossUp" | "maCrossDown" => {
            period(config.fast_ma, "fastMA")?.max(period(config.slow_ma, "slowMA")?) + 1
        }
        "emaCrossUp" | "emaCrossDown" => period(config.ema_period, "emaPeriod")? + 1,
        "priceAboveSlow" | "priceBelowSlow" => period(config.slow_ma, "slowMA")?,
        "rsiOversold" | "rsiOverbought" => period(config.rsi_period, "rsiPeriod")? + 1 + 1,
        "macdCrossUp" | "macdCrossDown" => macd_signal_lookback(config)? + 1,
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
    Ok(EmbargoDerivation {
        embargo_bars: lookback + holding_allowance_bars,
        max_signal_lookback_bars: lookback,
        holding_allowance_bars,
    })
}
