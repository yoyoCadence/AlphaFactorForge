// FULL — typed wrappers around Tauri `invoke`. The frontend talks to the
// backend ONLY through these functions. No direct SQLite, no direct AI calls.
//
// Uses @tauri-apps/api v2. In a non-Tauri context (e.g. plain `vite` in a
// browser for component dev), `isTauri()` is false and callers should guard.

import { invoke, isTauri } from '@tauri-apps/api/core';

export { isTauri };

// ---- shared shapes (mirror Rust DTOs / SQLite schema) ----
export interface Dataset {
  id?: number;
  exchange: string;
  symbol: string;
  interval: string;
  start_time: number;
  end_time: number;
  candle_count: number;
  source: string;
  dataset_hash: string;
}

export interface Candle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface StrategyDef {
  id?: number;
  name: string;
  type: 'params' | 'blocks' | 'code' | 'dsl' | 'ai_dsl';
  dsl_json?: string | null;
  original_definition_json: string;
  param_schema_json?: string | null;
  source: 'manual' | 'sweep' | 'traditional' | 'ai';
  ai_prompt_hash?: string | null;
  strategy_hash: string;
  lifecycle: 'candidate' | 'validated' | 'rejected';
  parent_strategy_id?: number | null;
}

// One backtest_summary row. snake_case mirrors the SQLite columns / Rust DTO.
// NOTE: core/metrics emits a camelCase `Metrics`; map it onto these fields when
// saving (e.g. netReturn -> net_return). gate_passed/score/*_json are Phase B.
export interface BacktestSummary {
  id?: number;
  strategy_id: number;
  dataset_id: number;
  segment: 'train' | 'validation' | 'test' | 'full';
  start_time: number;
  end_time: number;
  net_return?: number | null;
  cagr?: number | null;
  max_drawdown?: number | null;
  sharpe?: number | null;
  sortino?: number | null;
  calmar?: number | null;
  win_rate?: number | null;
  trade_count?: number | null;
  profit_factor?: number | null;
  avg_trade_return?: number | null;
  median_trade_return?: number | null;
  exposure?: number | null;
  turnover?: number | null;
  largest_win?: number | null;
  largest_loss?: number | null;
  consecutive_losses?: number | null;
  gate_passed?: boolean | null;
  score?: number | null;
  score_breakdown_json?: string | null;
  benchmark_result_json?: string | null;
  created_at?: string;
}

// One closed trade written under a backtest_summary row. `bars` is not part of
// the Phase A schema; fee/slippage remain NULL in Rust until recorded per trade.
export interface TradeRow {
  entry_time: number;
  exit_time: number;
  side: 'LONG' | 'SHORT';
  entry_price: number;
  exit_price: number;
  pnl: number;
  pnl_pct: number;
  reason: string | null;
}

// ---- Database ----
export const db = {
  init: () => invoke<string>('init_database'),
  runMigrations: () => invoke<string>('run_migrations'),
  getDatasets: () => invoke<Dataset[]>('get_datasets'),
  getCandles: (dataset_id: number, from: number, to: number) =>
    invoke<Candle[]>('get_candles', { datasetId: dataset_id, from, to }),
  importCandles: (dataset: Dataset, candles: Candle[]) =>
    invoke<number>('import_candles', { dataset, candles }),
  saveStrategy: (strategy: StrategyDef) => invoke<number>('save_strategy', { strategy }),
  getStrategies: () => invoke<StrategyDef[]>('get_strategies'),
  saveBacktestResult: (summary: BacktestSummary, trades: TradeRow[]) =>
    invoke<number>('save_backtest_result', { summary, trades }),
  getBacktestResults: (strategyId?: number) =>
    invoke<BacktestSummary[]>('get_backtest_results', { strategyId }),
};

// ---- Files ----
export const files = {
  saveReport: (suggestedFilename: string, contents: string) =>
    invoke<string>('save_report', { suggestedFilename, contents }),
};

// ---- Secrets (Phase C) ----
export const secrets = {
  saveKey: (provider: string, key: string) => invoke<void>('save_ai_api_key', { provider, key }),
  keyStatus: (provider: string) => invoke<boolean>('get_ai_api_key_status', { provider }),
  deleteKey: (provider: string) => invoke<void>('delete_ai_api_key', { provider }),
  testConnection: (provider: string) => invoke<boolean>('test_ai_connection', { provider }),
};

// ---- AI (Phase C) ----
export const ai = {
  generateDSL: (promptContext: unknown) =>
    invoke<unknown>('generate_strategy_dsl', { promptContext }),
  validateDSL: (dsl: unknown) => invoke<unknown>('validate_strategy_dsl', { dsl }),
};

// ---- Discovery (Phase B) ----
export const discovery = {
  start: (config: unknown) => invoke<number>('start_discovery', { config }),
  pause: (runId: number) => invoke<void>('pause_discovery', { runId }),
  resume: (runId: number) => invoke<void>('resume_discovery', { runId }),
  cancel: (runId: number) => invoke<void>('cancel_discovery', { runId }),
  progress: (runId: number) => invoke<unknown>('get_discovery_progress', { runId }),
};
