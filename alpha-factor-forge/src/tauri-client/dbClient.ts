// FULL — higher-level data client used by UI/stores. Wraps tauri-client
// commands and adds hashing so imports get a correct dataset_hash. Keeps
// hashing in the shared core module (single source of truth).

import { normalizeDatasetCandles, normalizedDatasetHash } from '../core/hashing';
import { db, type Candle, type Dataset } from './commands';

export interface ImportCandlesInput {
  exchange: string;
  symbol: string;
  interval: string;
  source: string;
  candles: Candle[];
}

export interface PreparedDatasetImport {
  dataset: Dataset;
  candles: Candle[];
}

/** Build the exact immutable payload sent across the Tauri boundary. */
export async function prepareDatasetImport(
  input: ImportCandlesInput,
): Promise<PreparedDatasetImport> {
  const candles = normalizeDatasetCandles(input.candles);
  const start_time = candles[0].timestamp;
  const end_time = candles[candles.length - 1].timestamp;
  const hash = await normalizedDatasetHash(
    {
      exchange: input.exchange,
      symbol: input.symbol,
      interval: input.interval,
    },
    candles,
  );
  return {
    candles,
    dataset: {
      exchange: input.exchange,
      symbol: input.symbol,
      interval: input.interval,
      start_time,
      end_time,
      candle_count: candles.length,
      source: input.source,
      dataset_hash: hash,
    },
  };
}

/** Compute the durable identity, then atomically import through the backend. */
export async function importDataset(input: ImportCandlesInput): Promise<number> {
  const prepared = await prepareDatasetImport(input);
  return db.importCandles(prepared.dataset, prepared.candles);
}

export const dbClient = { importDataset, ...db };
