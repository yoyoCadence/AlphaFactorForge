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
  saveBacktestResult: (resultJson: string) =>
    invoke<number>('save_backtest_result', { resultJson }),
  getBacktestResults: (strategyId?: number) =>
    invoke<unknown[]>('get_backtest_results', { strategyId }),
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
