// SKELETON — lightweight FRONTEND worker for single interactive backtests /
// short sweeps / chart indicator precompute. Phase A placeholder.
//
// HARD RULES (per spec §13):
//   - No DOM, no React state, no Canvas DOM, no SQLite, no AI calls.
//   - No function callbacks across the boundary — ONLY a jobId + event protocol.
//   - Heavy Strategy Discovery does NOT run here; it runs in the Tauri backend.
//
// Protocol: postMessage({ type, jobId, payload }) both ways.

import { runBacktest, type BacktestConfig, type Candle, type Signals } from '../core/backtest';

interface RunMsg {
  type: 'run';
  jobId: string;
  payload: { candles: Candle[]; signals: Signals; config: BacktestConfig };
}
type InMsg = RunMsg;

self.onmessage = (e: MessageEvent<InMsg>) => {
  const msg = e.data;
  if (msg.type === 'run') {
    try {
      const result = runBacktest(msg.payload.candles, msg.payload.signals, msg.payload.config);
      (self as unknown as Worker).postMessage({ type: 'result', jobId: msg.jobId, payload: result });
    } catch (err) {
      (self as unknown as Worker).postMessage({ type: 'error', jobId: msg.jobId, payload: String(err) });
    }
  }
};

export {};
