//! `params-signals-v1`: pure Rust parity port of the params-mode branch of
//! `src/services/strategySignals.ts`.
//!
//! Per the PR #66 Resolution D2, v1 discovery candidates are params-mode
//! only: blocks and code signal building stay TypeScript-only, and the safe
//! expression interpreter is deliberately not ported. Semantics mirror the
//! reference: bar 0 never signals, indicator warm-up `NaN` compares false,
//! and crosses require finite previous values on both sides.

use serde::Deserialize;

use super::backtest::Signals;
use super::indicators::{bbands, ema, macd, rsi, sma};
use super::types::Candle;

pub const PARAMS_SIGNALS_CONTRACT_VERSION: &str = "params-signals-v1";

/// The strategy fields the params signal builder reads (camelCase JSON,
/// matching the persisted TypeScript strategy shape).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamsSignalConfig {
    #[serde(rename = "fastMA")]
    pub fast_ma: usize,
    #[serde(rename = "slowMA")]
    pub slow_ma: usize,
    pub ema_period: usize,
    pub rsi_period: usize,
    pub rsi_buy: f64,
    pub rsi_sell: f64,
    pub macd_fast: usize,
    pub macd_slow: usize,
    pub macd_signal: usize,
    pub bb_period: usize,
    pub bb_mult: f64,
    pub entry_sig: String,
    pub exit_sig: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignalsError(pub String);

impl std::fmt::Display for SignalsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SignalsError {}

fn unsupported(id: &str) -> SignalsError {
    SignalsError(format!(
        "unsupported signal \"{id}\": stoch* signals await a core STOCH indicator (Phase B)"
    ))
}

#[derive(Clone, Copy)]
enum SignalKind {
    MaCrossUp,
    MaCrossDown,
    EmaCrossUp,
    EmaCrossDown,
    PriceAboveSlow,
    PriceBelowSlow,
    RsiOversold,
    RsiOverbought,
    MacdCrossUp,
    MacdCrossDown,
    BbLowerTouch,
    BbUpperTouch,
}

fn resolve(id: &str) -> Result<SignalKind, SignalsError> {
    Ok(match id {
        "maCrossUp" => SignalKind::MaCrossUp,
        "maCrossDown" => SignalKind::MaCrossDown,
        "emaCrossUp" => SignalKind::EmaCrossUp,
        "emaCrossDown" => SignalKind::EmaCrossDown,
        "priceAboveSlow" => SignalKind::PriceAboveSlow,
        "priceBelowSlow" => SignalKind::PriceBelowSlow,
        "rsiOversold" => SignalKind::RsiOversold,
        "rsiOverbought" => SignalKind::RsiOverbought,
        "macdCrossUp" => SignalKind::MacdCrossUp,
        "macdCrossDown" => SignalKind::MacdCrossDown,
        "bbLowerTouch" => SignalKind::BbLowerTouch,
        "bbUpperTouch" => SignalKind::BbUpperTouch,
        other => return Err(unsupported(other)),
    })
}

enum Operand<'a> {
    Series(&'a [f64]),
    Constant(f64),
}

fn at(operand: &Operand<'_>, index: i64) -> f64 {
    match operand {
        Operand::Constant(value) => *value,
        Operand::Series(series) => {
            if index >= 0 && (index as usize) < series.len() {
                series[index as usize]
            } else {
                f64::NAN
            }
        }
    }
}

#[derive(Clone, Copy)]
enum RuleOp {
    Gt,
    Lt,
    CrossUp,
    CrossDown,
}

/// Evaluate one comparison at bar `i`. `NaN` (warm-up) -> false; needs the
/// previous bar, so `i < 1` is always false (matches the reference).
fn eval_cond(left: &Operand<'_>, op: RuleOp, right: &Operand<'_>, index: i64) -> bool {
    if index < 1 {
        return false;
    }
    let a = at(left, index);
    let b = at(right, index);
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let previous_a = at(left, index - 1);
    let previous_b = at(right, index - 1);
    match op {
        RuleOp::Gt => a > b,
        RuleOp::Lt => a < b,
        RuleOp::CrossUp => {
            previous_a.is_finite() && previous_b.is_finite() && previous_a <= previous_b && a > b
        }
        RuleOp::CrossDown => {
            previous_a.is_finite() && previous_b.is_finite() && previous_a >= previous_b && a < b
        }
    }
}

struct SeriesSet {
    closes: Vec<f64>,
    ma_fast: Vec<f64>,
    ma_slow: Vec<f64>,
    ema: Vec<f64>,
    rsi: Vec<f64>,
    macd_line: Vec<f64>,
    macd_signal: Vec<f64>,
    bb_upper: Vec<f64>,
    bb_lower: Vec<f64>,
}

fn evaluate(kind: SignalKind, series: &SeriesSet, config: &ParamsSignalConfig, index: i64) -> bool {
    match kind {
        SignalKind::MaCrossUp => eval_cond(
            &Operand::Series(&series.ma_fast),
            RuleOp::CrossUp,
            &Operand::Series(&series.ma_slow),
            index,
        ),
        SignalKind::MaCrossDown => eval_cond(
            &Operand::Series(&series.ma_fast),
            RuleOp::CrossDown,
            &Operand::Series(&series.ma_slow),
            index,
        ),
        SignalKind::EmaCrossUp => eval_cond(
            &Operand::Series(&series.closes),
            RuleOp::CrossUp,
            &Operand::Series(&series.ema),
            index,
        ),
        SignalKind::EmaCrossDown => eval_cond(
            &Operand::Series(&series.closes),
            RuleOp::CrossDown,
            &Operand::Series(&series.ema),
            index,
        ),
        SignalKind::PriceAboveSlow => eval_cond(
            &Operand::Series(&series.closes),
            RuleOp::Gt,
            &Operand::Series(&series.ma_slow),
            index,
        ),
        SignalKind::PriceBelowSlow => eval_cond(
            &Operand::Series(&series.closes),
            RuleOp::Lt,
            &Operand::Series(&series.ma_slow),
            index,
        ),
        SignalKind::RsiOversold => eval_cond(
            &Operand::Series(&series.rsi),
            RuleOp::CrossUp,
            &Operand::Constant(config.rsi_buy),
            index,
        ),
        SignalKind::RsiOverbought => eval_cond(
            &Operand::Series(&series.rsi),
            RuleOp::CrossDown,
            &Operand::Constant(config.rsi_sell),
            index,
        ),
        SignalKind::MacdCrossUp => eval_cond(
            &Operand::Series(&series.macd_line),
            RuleOp::CrossUp,
            &Operand::Series(&series.macd_signal),
            index,
        ),
        SignalKind::MacdCrossDown => eval_cond(
            &Operand::Series(&series.macd_line),
            RuleOp::CrossDown,
            &Operand::Series(&series.macd_signal),
            index,
        ),
        SignalKind::BbLowerTouch => eval_cond(
            &Operand::Series(&series.closes),
            RuleOp::Lt,
            &Operand::Series(&series.bb_lower),
            index,
        ),
        SignalKind::BbUpperTouch => eval_cond(
            &Operand::Series(&series.closes),
            RuleOp::Gt,
            &Operand::Series(&series.bb_upper),
            index,
        ),
    }
}

/// Params-mode strategy -> entry/exit boolean signal arrays. Fails closed on
/// any unsupported signal id (including the `stoch*` family).
pub fn build_params_signals(
    candles: &[Candle],
    config: &ParamsSignalConfig,
) -> Result<Signals, SignalsError> {
    let entry_kind = resolve(&config.entry_sig)?;
    let exit_kind = resolve(&config.exit_sig)?;

    let closes: Vec<f64> = candles.iter().map(|candle| candle.close).collect();
    let macd_output = macd(
        &closes,
        config.macd_fast,
        config.macd_slow,
        config.macd_signal,
    );
    let bands = bbands(&closes, config.bb_period, config.bb_mult);
    let series = SeriesSet {
        ma_fast: sma(&closes, config.fast_ma),
        ma_slow: sma(&closes, config.slow_ma),
        ema: ema(&closes, config.ema_period),
        rsi: rsi(&closes, config.rsi_period),
        macd_line: macd_output.macd,
        macd_signal: macd_output.signal,
        bb_upper: bands.upper,
        bb_lower: bands.lower,
        closes,
    };

    let length = candles.len();
    let mut entry = Vec::with_capacity(length);
    let mut exit = Vec::with_capacity(length);
    for index in 0..length as i64 {
        entry.push(evaluate(entry_kind, &series, config, index));
        exit.push(evaluate(exit_kind, &series, config, index));
    }
    Ok(Signals { entry, exit })
}
