// FULL — higher-level data client used by UI/stores. Wraps tauri-client
// commands and adds hashing so imports get a correct dataset_hash. Keeps
// hashing in the shared core module (single source of truth).

import { datasetHash } from '../core/hashing';
import { db, type Candle, type Dataset } from './commands';

export interface ImportCandlesInput {
  exchange: string;
  symbol: string;
  interval: string;
  source: string;
  candles: Candle[];
}

/** Compute the dataset hash, then import the dataset + candles via backend. */
export async function importDataset(input: ImportCandlesInput): Promise<number> {
  const { candles } = input;
  if (!candles.length) throw new Error('no candles to import');
  const start_time = candles[0].timestamp;
  const end_time = candles[candles.length - 1].timestamp;
  const hash = await datasetHash({
    exchange: input.exchange,
    symbol: input.symbol,
    interval: input.interval,
    startTime: start_time,
    endTime: end_time,
  });
  const dataset: Dataset = {
    exchange: input.exchange,
    symbol: input.symbol,
    interval: input.interval,
    start_time,
    end_time,
    candle_count: candles.length,
    source: input.source,
    dataset_hash: hash,
  };
  return db.importCandles(dataset, candles);
}

export const dbClient = { importDataset, ...db };
