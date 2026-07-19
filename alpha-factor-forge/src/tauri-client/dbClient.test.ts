import { describe, expect, it } from 'vitest';
import fixture from '../core/hashing/identity-v2.fixture.json';
import { defaultStrategy } from '../services/strategy';
import { buildStrategyDef } from '../services/strategyRecord';
import { prepareDatasetImport } from './dbClient';
import { makeMockClient } from './mockClient';

const importInput = () => ({
  ...fixture.dataset.meta,
  source: 'identity-test',
  candles: fixture.dataset.candles.map((candle) => ({ ...candle })),
});

describe('durable identity client boundary', () => {
  it('prepares sorted content metadata and the committed v2 dataset hash', async () => {
    const prepared = await prepareDatasetImport(importInput());
    expect(prepared.dataset.dataset_hash).toBe(fixture.dataset.expectedHash);
    expect(prepared.dataset.start_time).toBe(1721001600000);
    expect(prepared.dataset.end_time).toBe(1721005200000);
    expect(prepared.dataset.candle_count).toBe(2);
    expect(prepared.candles.map((candle) => candle.timestamp)).toEqual([
      1721001600000,
      1721005200000,
    ]);
  });

  it('keeps mock imports idempotent and rejects a forged boundary payload', async () => {
    const client = makeMockClient();
    const first = await client.importDataset(importInput());
    const second = await client.importDataset(importInput());
    expect(second).toBe(first);
    expect(await client.db.getDatasets()).toHaveLength(1);

    const prepared = await prepareDatasetImport(importInput());
    await expect(client.db.importCandles({
      ...prepared.dataset,
      dataset_hash: 'dataset-content-v2:forged',
    }, prepared.candles)).rejects.toThrow(/identity/);
    expect(await client.db.getDatasets()).toHaveLength(1);
  });

  it('accepts verified strategies and rejects legacy or forged hashes', async () => {
    const client = makeMockClient();
    const definition = await buildStrategyDef(defaultStrategy(), 'v2 strategy');
    await expect(client.db.saveStrategy(definition)).resolves.toBeGreaterThan(0);
    await expect(client.db.saveStrategy({
      ...definition,
      strategy_hash: 'legacy-unversioned',
    })).rejects.toThrow(/identity/);
  });
});
