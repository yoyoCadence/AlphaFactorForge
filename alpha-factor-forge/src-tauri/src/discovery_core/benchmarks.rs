//! `benchmark-suite-v1`: pure Rust parity port of the deterministic §6
//! benchmark suite in `src/services/benchmarks.ts` (BENCH-001).
//!
//! Every benchmark runs long-only, 100% sizing, close fill, no risk exits,
//! inheriting only the candidate's fee/slippage. Buy & Hold uses hand-built
//! signals (enter the first tested bar's close, hold to EOD); the three
//! signal benchmarks build params-mode signals through the RS-CORE-003 port.
//! Their exact params strategy records are retained for audit persistence and
//! compared structurally against the TypeScript-reference fixture.

use serde::{Deserialize, Serialize};

use super::backtest::{
    run_backtest, BacktestConfig, BacktestError, BacktestResult, CostModel, Direction,
    ExecutionModel, FillMode, Signals,
};
use super::signals::{build_params_signals, ParamsSignalConfig};
use super::types::Candle;

pub const BENCHMARK_CONTRACT_VERSION: &str = "benchmark-suite-v1";

pub const DETERMINISTIC_BENCHMARK_IDS: [&str; 4] =
    ["buyHold", "smaCross", "rsiReversion", "bollingerReversion"];

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkCosts {
    pub fee_pct: f64,
    pub slip_pct: f64,
}

pub struct RunBenchmarksArgs<'a> {
    pub candles: &'a [Candle],
    pub interval: &'a str,
    pub costs: BenchmarkCosts,
    pub start_equity: Option<f64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BenchmarkRule {
    pub l: String,
    pub op: String,
    pub r: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkStrategy {
    pub mode: String,
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
    pub entry_rules: Vec<BenchmarkRule>,
    pub exit_rules: Vec<BenchmarkRule>,
    pub entry_code: String,
    pub exit_code: String,
    pub sl_pct: f64,
    pub tp_pct: f64,
    pub fee_pct: f64,
    pub slip_pct: f64,
    pub size_pct: f64,
    pub fill_mode: String,
    pub direction: String,
}

impl BenchmarkStrategy {
    fn signal_config(&self) -> ParamsSignalConfig {
        ParamsSignalConfig {
            fast_ma: self.fast_ma,
            slow_ma: self.slow_ma,
            ema_period: self.ema_period,
            rsi_period: self.rsi_period,
            rsi_buy: self.rsi_buy,
            rsi_sell: self.rsi_sell,
            macd_fast: self.macd_fast,
            macd_slow: self.macd_slow,
            macd_signal: self.macd_signal,
            bb_period: self.bb_period,
            bb_mult: self.bb_mult,
            entry_sig: self.entry_sig.clone(),
            exit_sig: self.exit_sig.clone(),
        }
    }
}

pub struct BenchmarkRun {
    pub id: &'static str,
    pub strat: Option<BenchmarkStrategy>,
    pub result: BacktestResult,
}

/// Approx. bars per year per interval (annualisation), unknown -> daily.
pub fn bars_per_year(interval: &str) -> f64 {
    match interval {
        "1m" => 525_600.0,
        "3m" => 175_200.0,
        "5m" => 105_120.0,
        "15m" => 35_040.0,
        "1h" => 8_760.0,
        "4h" => 2_190.0,
        "1d" => 365.0,
        _ => 365.0,
    }
}

/// Legacy percent-unit exec cost conversion + clamping (mirrors
/// `toExecCostFractions` with `sizePct = 100` -> sizing 1.0).
fn exec_cost_fractions(costs: BenchmarkCosts) -> (f64, f64, f64) {
    let fee = costs.fee_pct.max(0.0) / 100.0;
    let slip = costs.slip_pct.max(0.0) / 100.0;
    (fee, slip, 1.0)
}

fn benchmark_config(args: &RunBenchmarksArgs<'_>) -> BacktestConfig {
    let (fee_pct, slippage_pct, sizing_pct) = exec_cost_fractions(args.costs);
    BacktestConfig {
        exec: ExecutionModel {
            direction: Direction::Long,
            sizing_pct,
            fill_mode: FillMode::Close,
        },
        cost: CostModel {
            fee_pct,
            slippage_pct,
        },
        risk: None,
        start_equity: args.start_equity,
        bars_per_year: bars_per_year(args.interval),
        from: args.from,
        to: args.to,
    }
}

/// Full defaults shared with `defaultStrategy()` in TypeScript.
fn strategy_defaults(costs: BenchmarkCosts) -> BenchmarkStrategy {
    BenchmarkStrategy {
        mode: "params".into(),
        fast_ma: 9,
        slow_ma: 21,
        ema_period: 50,
        rsi_period: 14,
        rsi_buy: 30.0,
        rsi_sell: 70.0,
        macd_fast: 12,
        macd_slow: 26,
        macd_signal: 9,
        bb_period: 20,
        bb_mult: 2.0,
        entry_sig: "maCrossUp".into(),
        exit_sig: "maCrossDown".into(),
        entry_rules: vec![BenchmarkRule {
            l: "maFast".into(),
            op: "crossUp".into(),
            r: "maSlow".into(),
        }],
        exit_rules: vec![BenchmarkRule {
            l: "maFast".into(),
            op: "crossDown".into(),
            r: "maSlow".into(),
        }],
        entry_code: "crossUp(macd, macdSignal)".into(),
        exit_code: "crossDown(macd, macdSignal)".into(),
        sl_pct: 0.0,
        tp_pct: 0.0,
        fee_pct: costs.fee_pct,
        slip_pct: costs.slip_pct,
        size_pct: 100.0,
        fill_mode: "close".into(),
        direction: "long".into(),
    }
}

/// The exact params strategy one signal benchmark runs (doc §6).
fn benchmark_strategy(id: &str, costs: BenchmarkCosts) -> BenchmarkStrategy {
    let mut strategy = strategy_defaults(costs);
    match id {
        "smaCross" => {
            strategy.fast_ma = 50;
            strategy.slow_ma = 200;
        }
        "rsiReversion" => {
            strategy.rsi_period = 14;
            strategy.rsi_buy = 30.0;
            strategy.rsi_sell = 70.0;
            strategy.entry_sig = "rsiOversold".into();
            strategy.exit_sig = "rsiOverbought".into();
        }
        "bollingerReversion" => {
            strategy.bb_period = 20;
            strategy.bb_mult = 2.0;
            strategy.entry_sig = "bbLowerTouch".into();
            strategy.exit_sig = "bbUpperTouch".into();
        }
        other => unreachable!("benchmark_strategy called with {other}"),
    }
    strategy
}

/// Buy & Hold: enter at the first tested bar's close, never exit; the engine's
/// EOD settlement closes the position at the segment end.
fn run_buy_hold(args: &RunBenchmarksArgs<'_>) -> Result<BacktestResult, BacktestError> {
    let n = args.candles.len();
    let from = args.from.unwrap_or(0).max(0);
    let mut entry = vec![false; n];
    if from >= 0 && (from as usize) < n {
        entry[from as usize] = true;
    }
    let exit = vec![false; n];
    run_backtest(
        args.candles,
        &Signals { entry, exit },
        &benchmark_config(args),
    )
}

/// Run the four deterministic benchmarks over one candles × segment, in the
/// fixed order. Fails closed on an empty series.
pub fn run_deterministic_benchmarks(
    args: &RunBenchmarksArgs<'_>,
) -> Result<Vec<BenchmarkRun>, BacktestError> {
    if args.candles.is_empty() {
        return Err(BacktestError(
            "benchmarks need a non-empty candle series".into(),
        ));
    }
    let mut runs = Vec::with_capacity(DETERMINISTIC_BENCHMARK_IDS.len());
    for id in DETERMINISTIC_BENCHMARK_IDS {
        let (strat, result) = if id == "buyHold" {
            (None, run_buy_hold(args)?)
        } else {
            let strategy = benchmark_strategy(id, args.costs);
            let signals = build_params_signals(args.candles, &strategy.signal_config())
                .map_err(|error| BacktestError(error.to_string()))?;
            let result = run_backtest(args.candles, &signals, &benchmark_config(args))?;
            (Some(strategy), result)
        };
        runs.push(BenchmarkRun { id, strat, result });
    }
    Ok(runs)
}
