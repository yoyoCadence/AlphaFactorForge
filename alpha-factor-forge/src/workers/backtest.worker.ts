// Lightweight FRONTEND worker for interactive backtests and bounded sweeps.
// PERF-001 connects runSweep to the UI; heavy discovery remains in Rust.
//
// HARD RULES (per spec §13):
//   - No DOM, no React state, no Canvas DOM, no SQLite, no AI calls.
//   - No function callbacks across the boundary — ONLY a jobId + event protocol.
//   - Heavy Strategy Discovery does NOT run here; it runs in the Tauri backend.
//
// Protocol: postMessage({ type, jobId, payload }) both ways.

import { runBacktest, type BacktestConfig, type Candle, type Signals } from '../core/backtest';
import { runParamSweep, type RunParamSweepArgs, type SweepResult } from '../services/paramSweep';

interface RunMsg {
  type: 'run';
  jobId: string;
  payload: { candles: Candle[]; signals: Signals; config: BacktestConfig };
}
export type BacktestWorkerRequest = RunMsg | {
  type: 'runSweep';
  jobId: string;
  payload: RunParamSweepArgs;
};
export type BacktestWorkerResponse =
  | { type: 'result'; jobId: string; payload: ReturnType<typeof runBacktest> }
  | { type: 'sweepResult'; jobId: string; payload: SweepResult }
  | { type: 'error'; jobId: string; payload: string };

self.onmessage = (e: MessageEvent<BacktestWorkerRequest>) => {
  const msg = e.data;
  if (msg.type === 'run') {
    try {
      const result = runBacktest(msg.payload.candles, msg.payload.signals, msg.payload.config);
      (self as unknown as Worker).postMessage({ type: 'result', jobId: msg.jobId, payload: result });
    } catch (err) {
      (self as unknown as Worker).postMessage({ type: 'error', jobId: msg.jobId, payload: String(err) });
    }
  }
  else if (msg.type === 'runSweep') {
    try {
      const result = runParamSweep(msg.payload);
      (self as unknown as Worker).postMessage({ type: 'sweepResult', jobId: msg.jobId, payload: result });
    } catch (err) {
      (self as unknown as Worker).postMessage({ type: 'error', jobId: msg.jobId, payload: String(err) });
    }
  }
};

export {};
