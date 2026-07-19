// DEV/TEST-ONLY in-memory mock of the Tauri data boundary.
//
// Activated only through the `dataClient` seam (`?mock=1` in Vite dev) so that
// browser E2E (Playwright) can drive the real React UI without a Tauri backend.
// This is NOT a product path: state lives in memory only — no localStorage, no
// real SQLite — and it is never used in a Tauri/production build. It does NOT
// replace real Tauri/Rust/SQLite verification (Rust integration tests +
// `cargo tauri dev` smoke still own that).

import type {
  Candle,
  Dataset,
  StrategyDef,
  BacktestSummary,
  TradeRow,
  ValidationRecordRow,
} from './commands';
import { prepareDatasetImport, type ImportCandlesInput } from './dbClient';
import { assertValidBundle } from '../services/validationRecord';
import { strategyHashFromDefinitionJson } from '../core/hashing';

export function makeMockClient() {
  const datasets: Dataset[] = [];
  const candlesByDs = new Map<number, Candle[]>();
  const strategies: StrategyDef[] = [];
  const summaries: BacktestSummary[] = [];
  // There is no trades reader yet, but the E2E seam still mirrors SQLite's
  // replace-on-summary-key persistence instead of silently dropping the rows.
  const tradesBySummaryId = new Map<number, TradeRow[]>();
  const validationRecords: ValidationRecordRow[] = [];
  let nextId = 1;

  const db = {
    init: async () => 'mock database ready',
    runMigrations: async () => 'mock: migrations up to date',
    getDatasets: async () => datasets.slice(),
    getCandles: async (datasetId: number, from: number, to: number) =>
      (candlesByDs.get(datasetId) ?? []).filter((c) => c.timestamp >= from && c.timestamp <= to),
    importCandles: async (dataset: Dataset, rows: Candle[]) => {
      const prepared = await prepareDatasetImport({
        exchange: dataset.exchange,
        symbol: dataset.symbol,
        interval: dataset.interval,
        source: dataset.source,
        candles: rows,
      });
      if (
        dataset.dataset_hash !== prepared.dataset.dataset_hash
        || dataset.start_time !== prepared.dataset.start_time
        || dataset.end_time !== prepared.dataset.end_time
        || dataset.candle_count !== prepared.dataset.candle_count
      ) {
        throw new Error('dataset identity or derived metadata mismatch');
      }
      const existing = datasets.find((row) => row.dataset_hash === dataset.dataset_hash);
      if (existing) {
        const existingRows = candlesByDs.get(existing.id!) ?? [];
        const sameDataset = existing.exchange === dataset.exchange
          && existing.symbol === dataset.symbol
          && existing.interval === dataset.interval
          && existing.start_time === dataset.start_time
          && existing.end_time === dataset.end_time
          && existing.candle_count === dataset.candle_count
          && existing.source === dataset.source;
        const sameRows = JSON.stringify(existingRows) === JSON.stringify(prepared.candles);
        if (!sameDataset || !sameRows) throw new Error('dataset hash conflicts with stored payload');
        return existing.id!;
      }
      const id = nextId++;
      datasets.push({ ...dataset, id });
      candlesByDs.set(id, prepared.candles.map((row) => ({ ...row })));
      return id;
    },
    saveStrategy: async (def: StrategyDef) => {
      const expectedHash = await strategyHashFromDefinitionJson(def.original_definition_json);
      const parsed = JSON.parse(def.original_definition_json) as Record<string, unknown>;
      if (def.strategy_hash !== expectedHash || def.type !== parsed.mode) {
        throw new Error('strategy identity mismatch');
      }
      const id = nextId++;
      strategies.push({ ...def, id });
      return id;
    },
    getStrategies: async () => strategies.slice(),
    saveBacktestResult: async (summary: BacktestSummary, trades: TradeRow[]) => {
      const existingIndex = summaries.findIndex(
        (row) => row.strategy_id === summary.strategy_id
          && row.dataset_id === summary.dataset_id
          && row.segment === summary.segment,
      );
      const existingId = existingIndex >= 0 ? summaries[existingIndex].id : undefined;
      const id = existingId ?? nextId++;
      const stored = { ...summary, id };
      if (existingIndex >= 0) summaries[existingIndex] = stored;
      else summaries.push(stored);
      tradesBySummaryId.set(id, trades.map((trade) => ({ ...trade })));
      return id;
    },
    getBacktestResults: async (strategyId?: number) =>
      summaries.filter((s) => strategyId == null || s.strategy_id === strategyId),
    // PERSIST-001 parity: runs the SAME shared bundle validator the composer
    // targets (the TS mirror of Rust's validate_validation_bundle), so
    // `?mock=1` rejects exactly the bundles native Tauri rejects, then
    // applies every write all-or-nothing.
    saveValidationRecord: async (
      trainSummary: BacktestSummary,
      trainTrades: TradeRow[],
      validationSummary: BacktestSummary,
      validationTrades: TradeRow[],
      record: ValidationRecordRow,
    ) => {
      assertValidBundle({ trainSummary, trainTrades, validationSummary, validationTrades, record });
      await db.saveBacktestResult(trainSummary, trainTrades);
      await db.saveBacktestResult(validationSummary, validationTrades);
      const id = nextId++;
      validationRecords.push({ ...record, id, created_at: new Date().toISOString() });
      return id;
    },
    // Reads return DETACHED copies so callers can never mutate the mock's
    // append-only records in place (PR #65 review).
    listValidationRecords: async (strategyId?: number) =>
      validationRecords
        .filter((r) => strategyId == null || r.strategy_id === strategyId)
        .map((r) => ({ ...r }))
        .reverse(),
    getValidationRecord: async (id: number) => {
      const row = validationRecords.find((r) => r.id === id);
      if (!row) throw new Error(`no validation record ${id}`);
      return { ...row };
    },
  };

  const files = {
    saveReport: async (suggestedFilename: string, contents: string) => {
      if (typeof document === 'undefined' || typeof Blob === 'undefined') {
        return `mock-download:${suggestedFilename}`;
      }
      const blob = new Blob([contents], { type: 'text/plain;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = suggestedFilename;
      a.style.display = 'none';
      document.body.appendChild(a);
      a.click();
      a.remove();
      globalThis.setTimeout(() => URL.revokeObjectURL(url), 0);
      return `browser-download:${suggestedFilename}`;
    },
  };

  const importDataset = async (input: ImportCandlesInput): Promise<number> => {
    const prepared = await prepareDatasetImport(input);
    return db.importCandles(prepared.dataset, prepared.candles);
  };

  return { db, files, importDataset, isTauri: () => true };
}
