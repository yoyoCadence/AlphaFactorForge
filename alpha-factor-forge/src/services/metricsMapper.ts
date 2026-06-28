// THE single camelCase Metrics -> snake_case BacktestSummary mapping.
//
// Per the PR #1 decision: do NOT inline this field mapping in components — every
// persistence call site goes through metricsToBacktestSummary() so the shape
// stays in one place. core/metrics emits camelCase; the SQLite columns / Rust
// DTO are snake_case. gate_passed / score / *_json are Phase B and stay unset.

import type { Metrics } from '../core/metrics';
import type { BacktestSummary } from '../tauri-client/commands';

export type Segment = BacktestSummary['segment'];

export interface SummaryKeys {
  strategyId: number;
  datasetId: number;
  /** time-segment label; defaults to 'full'. */
  segment?: Segment;
  /** epoch ms of the first / last candle in the tested range. */
  startTime: number;
  endTime: number;
}

/** JSON can't carry Infinity/NaN; persist non-finite metrics as null. */
const finite = (x: number): number | null => (Number.isFinite(x) ? x : null);

export function metricsToBacktestSummary(metrics: Metrics, keys: SummaryKeys): BacktestSummary {
  return {
    strategy_id: keys.strategyId,
    dataset_id: keys.datasetId,
    segment: keys.segment ?? 'full',
    start_time: keys.startTime,
    end_time: keys.endTime,
    net_return: finite(metrics.netReturn),
    cagr: finite(metrics.cagr),
    max_drawdown: finite(metrics.maxDrawdown),
    sharpe: finite(metrics.sharpe),
    sortino: finite(metrics.sortino),
    calmar: finite(metrics.calmar),
    win_rate: finite(metrics.winRate),
    trade_count: metrics.tradeCount,
    profit_factor: finite(metrics.profitFactor),
    avg_trade_return: finite(metrics.avgTradeReturn),
    median_trade_return: finite(metrics.medianTradeReturn),
    exposure: finite(metrics.exposure),
    turnover: finite(metrics.turnover),
    largest_win: finite(metrics.largestWin),
    largest_loss: finite(metrics.largestLoss),
    consecutive_losses: metrics.consecutiveLosses,
    // gate_passed / score / score_breakdown_json / benchmark_result_json: Phase B.
  };
}
