//! `backtest-execution-v1`: pure Rust parity port of `src/core/backtest`.
//!
//! Semantics mirror the TypeScript reference engine exactly, per
//! docs/backtest-execution-contract.md and the BUG-002/003/004 corrections:
//! fee-inclusive entry budgeting, unleveraged 1x short collateral, nextOpen
//! pending fills on the following tested candle (a final-bar signal never
//! fills), gap-aware SL/TP with conservative SL-first ambiguity, legacy
//! `both` reversal (entry requests LONG, exit requests SHORT, entry wins a
//! simultaneous bar), settled EOD equity endpoint, and fail-closed
//! normalized-fraction validation with the same error messages.

use serde::Deserialize;

use super::metrics::{compute_metrics, ClosedTrade, EquityPoint, Metrics, MetricsInput, TradeSide};
use super::types::Candle;

pub const EXECUTION_CONTRACT_VERSION: &str = "backtest-execution-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Long,
    Short,
    Both,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum FillMode {
    Close,
    NextOpen,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionModel {
    pub direction: Direction,
    /// Normalized fraction [0, 1]; 1 = all-in.
    pub sizing_pct: f64,
    pub fill_mode: FillMode,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostModel {
    /// Per side, normalized fraction [0, 1] (0.0005 = 0.05%).
    pub fee_pct: f64,
    /// Per side, normalized fraction [0, 1].
    pub slippage_pct: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskModel {
    /// Active normalized fraction (0, 1]; None = no stop.
    #[serde(default)]
    pub stop_loss_pct: Option<f64>,
    #[serde(default)]
    pub take_profit_pct: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Signals {
    /// entry[i] true => open in the execution direction; `both` requests LONG.
    pub entry: Vec<bool>,
    /// exit[i] true => close long/short mode; `both` requests SHORT.
    pub exit: Vec<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestConfig {
    pub exec: ExecutionModel,
    pub cost: CostModel,
    #[serde(default)]
    pub risk: Option<RiskModel>,
    #[serde(default)]
    pub start_equity: Option<f64>,
    pub bars_per_year: f64,
    /// Restrict the test to [from, to] index range (inclusive).
    #[serde(default)]
    pub from: Option<i64>,
    #[serde(default)]
    pub to: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct BacktestResult {
    pub trades: Vec<ClosedTrade>,
    pub equity: Vec<EquityPoint>,
    pub metrics: Metrics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BacktestError(pub String);

impl std::fmt::Display for BacktestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for BacktestError {}

struct OpenPos {
    side: TradeSide,
    entry_idx: i64,
    entry_time: i64,
    entry_price: f64,
    entry_notional: f64,
    entry_fee: f64,
    qty: f64,
}

struct PendingNextOpen {
    exit: bool,
    entry_side: Option<TradeSide>,
}

/// Apply per-side slippage to a fill price (buys pay up, sells receive less).
fn fill(price: f64, buy: bool, slip: f64) -> f64 {
    if buy {
        price * (1.0 + slip)
    } else {
        price * (1.0 - slip)
    }
}

fn assert_normalized_fraction(
    path: &str,
    value: f64,
    allow_zero: bool,
) -> Result<(), BacktestError> {
    let below_minimum = if allow_zero { value < 0.0 } else { value <= 0.0 };
    if !value.is_finite() || below_minimum || value > 1.0 {
        let range = if allow_zero { "[0, 1]" } else { "(0, 1]" };
        return Err(BacktestError(format!(
            "BacktestConfig.{path} must be a finite normalized fraction in {range}"
        )));
    }
    Ok(())
}

fn validate_normalized_fractions(cfg: &BacktestConfig) -> Result<(), BacktestError> {
    assert_normalized_fraction("exec.sizingPct", cfg.exec.sizing_pct, true)?;
    assert_normalized_fraction("cost.feePct", cfg.cost.fee_pct, true)?;
    assert_normalized_fraction("cost.slippagePct", cfg.cost.slippage_pct, true)?;
    if let Some(risk) = &cfg.risk {
        if let Some(stop_loss) = risk.stop_loss_pct {
            assert_normalized_fraction("risk.stopLossPct", stop_loss, false)?;
        }
        if let Some(take_profit) = risk.take_profit_pct {
            assert_normalized_fraction("risk.takeProfitPct", take_profit, false)?;
        }
    }
    Ok(())
}

struct EngineState {
    cash: f64,
    pos: Option<OpenPos>,
    trades: Vec<ClosedTrade>,
    fee_pct: f64,
    sizing_pct: f64,
}

impl EngineState {
    fn close(&mut self, candles: &[Candle], index: i64, price: f64) {
        let Some(pos) = self.pos.take() else {
            return;
        };
        let exit_notional = price * pos.qty;
        let exit_fee = exit_notional * self.fee_pct;
        let gross_pnl = match pos.side {
            TradeSide::Long => (price - pos.entry_price) * pos.qty,
            TradeSide::Short => (pos.entry_price - price) * pos.qty,
        };
        let pnl = gross_pnl - pos.entry_fee - exit_fee;
        match pos.side {
            TradeSide::Long => self.cash += exit_notional - exit_fee,
            // Phase A short = unleveraged 1x collateral + realised price PnL.
            TradeSide::Short => self.cash += pos.entry_notional + gross_pnl - exit_fee,
        }
        self.trades.push(ClosedTrade {
            entry_time: pos.entry_time,
            exit_time: candles[index as usize].timestamp,
            side: pos.side,
            entry_price: pos.entry_price,
            exit_price: price,
            pnl,
            pnl_pct: if pos.entry_notional > 0.0 {
                pnl / pos.entry_notional
            } else {
                0.0
            },
            bars: index - pos.entry_idx,
        });
    }

    fn open(&mut self, candles: &[Candle], index: i64, side: TradeSide, price: f64) {
        let budget = self.cash * self.sizing_pct;
        // sizingPct budgets entry notional + its fee, so 100% sizing never
        // makes free cash negative merely to pay the entry commission.
        let entry_notional = budget / (1.0 + self.fee_pct);
        let entry_fee = entry_notional * self.fee_pct;
        let qty = if price > 0.0 {
            entry_notional / price
        } else {
            0.0
        };
        if qty <= 0.0 {
            return;
        }
        self.cash -= entry_notional + entry_fee;
        self.pos = Some(OpenPos {
            side,
            entry_idx: index,
            entry_time: candles[index as usize].timestamp,
            entry_price: price,
            entry_notional,
            entry_fee,
            qty,
        });
    }
}

fn signal_at(signals: &[bool], index: i64) -> bool {
    usize::try_from(index)
        .ok()
        .and_then(|i| signals.get(i).copied())
        .unwrap_or(false)
}

/// Run the backtest. Deterministic: identical inputs -> identical output.
pub fn run_backtest(
    candles: &[Candle],
    signals: &Signals,
    cfg: &BacktestConfig,
) -> Result<BacktestResult, BacktestError> {
    validate_normalized_fractions(cfg)?;
    let start = cfg.start_equity.unwrap_or(10_000.0);
    let last_index = candles.len() as i64 - 1;
    let from = cfg.from.unwrap_or(0).max(0);
    let to = cfg.to.unwrap_or(last_index).min(last_index);
    let fee_pct = cfg.cost.fee_pct;
    let slippage_pct = cfg.cost.slippage_pct;
    let direction = cfg.exec.direction;
    let fill_mode = cfg.exec.fill_mode;
    let stop_loss = cfg.risk.as_ref().and_then(|risk| risk.stop_loss_pct);
    let take_profit = cfg.risk.as_ref().and_then(|risk| risk.take_profit_pct);

    let mut state = EngineState {
        cash: start,
        pos: None,
        trades: Vec::new(),
        fee_pct,
        sizing_pct: cfg.exec.sizing_pct,
    };
    let mut equity: Vec<EquityPoint> = Vec::new();
    let mut pending_next_open: Option<PendingNextOpen> = None;

    let requested_entry_side = || {
        if direction == Direction::Short {
            TradeSide::Short
        } else {
            TradeSide::Long
        }
    };
    let requested_both_side = |index: i64| -> Option<TradeSide> {
        if signal_at(&signals.entry, index) {
            Some(TradeSide::Long)
        } else if signal_at(&signals.exit, index) {
            Some(TradeSide::Short)
        } else {
            None
        }
    };

    let mut index = from;
    while index <= to {
        let candle = &candles[index as usize];

        // Signals from the previous bar become fills only now, at this bar's
        // open. This keeps nextOpen prices out of the prior signal bar's
        // equity point.
        if fill_mode == FillMode::NextOpen {
            if let Some(pending) = pending_next_open.take() {
                if pending.exit {
                    if let Some(side) = state.pos.as_ref().map(|pos| pos.side) {
                        let sell_side = side == TradeSide::Short;
                        state.close(candles, index, fill(candle.open, sell_side, slippage_pct));
                    }
                }
                if let Some(side) = pending.entry_side {
                    if state.pos.is_none() {
                        state.open(
                            candles,
                            index,
                            side,
                            fill(candle.open, side == TradeSide::Long, slippage_pct),
                        );
                    }
                }
            }
        }

        // intrabar SL/TP check (approximate: order unknown without sub-bars)
        let risk_snapshot = state.pos.as_ref().map(|pos| (pos.side, pos.entry_price));
        if let Some((side, entry_price)) = risk_snapshot {
            if stop_loss.is_some() || take_profit.is_some() {
                let is_long = side == TradeSide::Long;
                let sl_price = stop_loss.map(|fraction| {
                    if is_long {
                        entry_price * (1.0 - fraction)
                    } else {
                        entry_price * (1.0 + fraction)
                    }
                });
                let tp_price = take_profit.map(|fraction| {
                    if is_long {
                        entry_price * (1.0 + fraction)
                    } else {
                        entry_price * (1.0 - fraction)
                    }
                });
                let sl_hit = sl_price.map_or(false, |price| {
                    (is_long && candle.low <= price) || (!is_long && candle.high >= price)
                });
                if sl_hit {
                    let price = sl_price.expect("checked above");
                    let base = if is_long {
                        candle.open.min(price)
                    } else {
                        candle.open.max(price)
                    };
                    state.close(candles, index, fill(base, !is_long, slippage_pct));
                } else {
                    let tp_hit = tp_price.map_or(false, |price| {
                        (is_long && candle.high >= price) || (!is_long && candle.low <= price)
                    });
                    if tp_hit {
                        let price = tp_price.expect("checked above");
                        let base = if is_long {
                            candle.open.max(price)
                        } else {
                            candle.open.min(price)
                        };
                        state.close(candles, index, fill(base, !is_long, slippage_pct));
                    }
                }
            }
        }

        if fill_mode == FillMode::Close {
            if direction == Direction::Both {
                let desired = requested_both_side(index);
                if let Some(desired_side) = desired {
                    let current_side = state.pos.as_ref().map(|pos| pos.side);
                    if current_side != Some(desired_side) {
                        if current_side.is_some() {
                            let sell_side = current_side == Some(TradeSide::Short);
                            state.close(candles, index, fill(candle.close, sell_side, slippage_pct));
                        }
                        state.open(
                            candles,
                            index,
                            desired_side,
                            fill(candle.close, desired_side == TradeSide::Long, slippage_pct),
                        );
                    }
                }
            } else {
                if state.pos.is_some() && signal_at(&signals.exit, index) {
                    let sell_side = state.pos.as_ref().map(|pos| pos.side) == Some(TradeSide::Short);
                    state.close(candles, index, fill(candle.close, sell_side, slippage_pct));
                }
                if state.pos.is_none() && signal_at(&signals.entry, index) {
                    let side = requested_entry_side();
                    state.open(
                        candles,
                        index,
                        side,
                        fill(candle.close, side == TradeSide::Long, slippage_pct),
                    );
                }
            }
        } else if index < to {
            if direction == Direction::Both {
                if let Some(desired_side) = requested_both_side(index) {
                    let current_side = state.pos.as_ref().map(|pos| pos.side);
                    if current_side != Some(desired_side) {
                        pending_next_open = Some(PendingNextOpen {
                            exit: current_side.is_some(),
                            entry_side: Some(desired_side),
                        });
                    }
                }
            } else {
                let has_position = state.pos.is_some();
                let exit = has_position && signal_at(&signals.exit, index);
                let entry_side = if signal_at(&signals.entry, index) && (!has_position || exit) {
                    Some(requested_entry_side())
                } else {
                    None
                };
                if exit || entry_side.is_some() {
                    pending_next_open = Some(PendingNextOpen { exit, entry_side });
                }
            }
        }

        // mark-to-market equity
        let mut mark = state.cash;
        if let Some(pos) = &state.pos {
            mark += match pos.side {
                TradeSide::Long => candle.close * pos.qty,
                TradeSide::Short => {
                    pos.entry_notional + (pos.entry_price - candle.close) * pos.qty
                }
            };
        }
        equity.push(EquityPoint {
            time: candle.timestamp,
            equity: mark,
        });

        index += 1;
    }

    // Force-close at the end using the normal closing side of slippage. A
    // final nextOpen signal has no execution candle and cannot create a fill.
    if let Some(side) = state.pos.as_ref().map(|pos| pos.side) {
        let sell_side = side == TradeSide::Short;
        let last_close = candles[to as usize].close;
        state.close(candles, to, fill(last_close, sell_side, slippage_pct));
    }
    // The curve keeps one point per tested candle, but its endpoint must be
    // the settled account value (including EOD exit fee) used by metrics.
    if let Some(last) = equity.last_mut() {
        last.equity = state.cash;
    }

    let metrics = compute_metrics(&MetricsInput {
        trades: &state.trades,
        equity: &equity,
        start_equity: Some(start),
        total_bars: to - from + 1,
        bars_per_year: cfg.bars_per_year,
        risk_free_per_bar: None,
    });

    Ok(BacktestResult {
        trades: state.trades,
        equity,
        metrics,
    })
}
