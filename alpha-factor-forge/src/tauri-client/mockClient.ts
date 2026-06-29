// DEV/TEST-ONLY in-memory mock of the Tauri data boundary.
//
// Activated only through the `dataClient` seam (`?mock=1` in Vite dev) so that
// browser E2E (Playwright) can drive the real React UI without a Tauri backend.
// This is NOT a product path: state lives in memory only — no localStorage, no
// real SQLite — and it is never used in a Tauri/production build. It does NOT
// replace real Tauri/Rust/SQLite verification (Rust integration tests +
// `cargo tauri dev` smoke still own that).

import type { Candle, Dataset, StrategyDef, BacktestSummary } from './commands';
import type { ImportCandlesInput } from './dbClient';

export function makeMockClient() {
  const datasets: Dataset[] = [];
  const candlesByDs = new Map<number, Candle[]>();
  const strategies: StrategyDef[] = [];
  const summaries: BacktestSummary[] = [];
  let nextId = 1;

  const db = {
    init: async () => 'mock database ready',
    runMigrations: async () => 'mock: migrations up to date',
    getDatasets: async () => datasets.slice(),
    getCandles: async (datasetId: number, from: number, to: number) =>
      (candlesByDs.get(datasetId) ?? []).filter((c) => c.timestamp >= from && c.timestamp <= to),
    importCandles: async (dataset: Dataset, rows: Candle[]) => {
      const id = nextId++;
      datasets.push({ ...dataset, id });
      candlesByDs.set(id, rows.slice());
      return id;
    },
    saveStrategy: async (def: StrategyDef) => {
      const id = nextId++;
      strategies.push({ ...def, id });
      return id;
    },
    getStrategies: async () => strategies.slice(),
    saveBacktestResult: async (summary: BacktestSummary) => {
      const id = nextId++;
      summaries.push({ ...summary, id });
      return id;
    },
    getBacktestResults: async (strategyId?: number) =>
      summaries.filter((s) => strategyId == null || s.strategy_id === strategyId),
  };

  const importDataset = async (input: ImportCandlesInput): Promise<number> => {
    const rows = input.candles;
    if (!rows.length) throw new Error('no candles to import');
    const id = nextId++;
    datasets.push({
      id,
      exchange: input.exchange,
      symbol: input.symbol,
      interval: input.interval,
      start_time: rows[0].timestamp,
      end_time: rows[rows.length - 1].timestamp,
      candle_count: rows.length,
      source: input.source,
      dataset_hash: `mock-${input.symbol}-${input.interval}-${id}`,
    });
    candlesByDs.set(id, rows.slice());
    return id;
  };

  return { db, importDataset, isTauri: () => true };
}
